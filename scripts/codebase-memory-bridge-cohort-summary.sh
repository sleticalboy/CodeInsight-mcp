#!/usr/bin/env bash
set -euo pipefail

OUTPUT="/tmp/codeinsight-codebase-memory-bridge-cohort.md"
JSON_OUTPUT=""
MIN_REPORTS=1
MIN_BACKEND_AGREEMENT_RATE=100
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
  --min-backend-agreement-rate N
                      Minimum percent of reports whose backend_route_agreement.status is agree.
                      Default: 100.
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
      --min-backend-agreement-rate)
        [ "$#" -ge 2 ] || fail "--min-backend-agreement-rate requires a number"
        MIN_BACKEND_AGREEMENT_RATE="$2"
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
  case "$MIN_BACKEND_AGREEMENT_RATE" in
    ''|*[!0-9]*) fail "--min-backend-agreement-rate must be an integer from 0 to 100" ;;
  esac
  [ "$MIN_BACKEND_AGREEMENT_RATE" -le 100 ] ||
    fail "--min-backend-agreement-rate must be <= 100"

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
    --argjson min_reports "$MIN_REPORTS" \
    --argjson min_backend_agreement_rate "$MIN_BACKEND_AGREEMENT_RATE" '
      def derived_agreement_status($summary):
        ($summary.agreement.backend_route_agreement_status //
         $summary.route.backend_route_agreement.status //
         (if ($summary.agreement.first_file_matches_backend_top // false)
          then "agree"
          elif ($summary.agreement.first_file_in_backend_candidates // false)
          then "overlap"
          else "conflict"
          end));

      def derived_agreement_action($summary):
        ($summary.agreement.backend_route_agreement_recommended_action //
         $summary.route.backend_route_agreement.recommended_action //
         (if ($summary.agreement.first_file_matches_backend_top // false)
          then "read_selected_context"
          elif ($summary.agreement.first_file_in_backend_candidates // false)
          then "read_selected_context_then_compare_backend_rank"
          else "compare_backend_route_before_edits"
          end));

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
          backend_route_agreement_status: derived_agreement_status($summary),
          backend_route_agreement_recommended_action: derived_agreement_action($summary),
          backend_route_agreement_message: ($summary.route.backend_route_agreement.message // ""),
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
      ($reports | map(select(.backend_route_agreement_status == "agree")) | length) as $backend_agree_count |
      ($reports | map(select(.backend_route_agreement_status == "overlap")) | length) as $backend_overlap_count |
      ($reports | map(select(.backend_route_agreement_status == "conflict")) | length) as $backend_conflict_count |
      ($reports | map(select(.backend_route_agreement_status == "backend_only")) | length) as $backend_only_count |
      ($reports | map(select(.backend_route_agreement_status == "no_local_route")) | length) as $no_local_route_count |
      ($reports | map(select(.backend_route_agreement_status == "backend_without_candidates")) | length) as $backend_without_candidates_count |
      ($reports | map(select(.first_file_in_backend_candidates)) | length) as $candidate_match_count |
      ($reports | map(.selected_backend_candidate_count) | add // 0) as $selected_backend_total |
      ($reports | map(.backend_candidate_count) | add // 0) as $backend_candidate_total |
      ($reports | map(select((.status != "pass") or (.backend_route_agreement_status != "agree")))) as $conflicts |
      (if $report_count > 0 then (($backend_agree_count * 10000 / $report_count | floor) / 100) else 0 end) as $backend_agreement_rate |
      {
        status: (if $report_count < $min_reports then "insufficient_reports" elif $backend_agreement_rate < $min_backend_agreement_rate then "needs_review" elif ($conflicts | length) > 0 then "needs_review" else "pass" end),
        report_count: $report_count,
        min_reports: $min_reports,
        min_backend_agreement_rate: $min_backend_agreement_rate,
        pass_count: $pass_count,
        warn_count: $warn_count,
        backend_route_agreement_rate: $backend_agreement_rate,
        backend_route_agreement_counts: {
          agree: $backend_agree_count,
          overlap: $backend_overlap_count,
          conflict: $backend_conflict_count,
          backend_only: $backend_only_count,
          no_local_route: $no_local_route_count,
          backend_without_candidates: $backend_without_candidates_count,
          other: ($report_count - $backend_agree_count - $backend_overlap_count - $backend_conflict_count - $backend_only_count - $no_local_route_count - $backend_without_candidates_count)
        },
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
        next_action: (
          if $report_count < $min_reports then "collect_more_bridge_reports"
          elif $backend_only_count > 0 or $no_local_route_count > 0 then "review_backend_only_routes"
          elif $backend_conflict_count > 0 then "review_backend_local_conflicts"
          elif $backend_overlap_count > 0 then "review_backend_order_vs_local_route"
          elif $backend_agreement_rate < $min_backend_agreement_rate then "raise_backend_agreement_rate"
          else "run_more_real_backend_tasks"
          end)
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
    "- Backend agreement rate: " + tick((.backend_route_agreement_rate | tostring) + "%"),
    "- Backend agreement gate: " + tick((.min_backend_agreement_rate | tostring) + "%"),
    "- Pass reports: " + tick(.pass_count),
    "- Warn reports: " + tick(.warn_count),
    "- Backend agreement counts: " + tick("agree=" + (.backend_route_agreement_counts.agree | tostring) + ", overlap=" + (.backend_route_agreement_counts.overlap | tostring) + ", conflict=" + (.backend_route_agreement_counts.conflict | tostring) + ", backend_only=" + (.backend_route_agreement_counts.backend_only | tostring)),
    "- First-file top match rate: " + tick((.first_file_top_match_rate | tostring) + "%"),
    "- First-file candidate match rate: " + tick((.first_file_candidate_match_rate | tostring) + "%"),
    "- Selected backend candidate rate: " + tick((.selected_backend_candidate_rate | tostring) + "%"),
    "- Next action: " + tick(.next_action),
    "",
    "| Task | Status | Backend agreement | Backend top | Route first | Top match | Candidate match | Route quality | Agent route action | Warnings | Next action |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    (.reports[] |
      "| " + (if .task == "" then "-" else .task end) +
      " | " + tick(.status) +
      " | " + tick(.backend_route_agreement_status) +
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
  local agreement_rate
  status="$(jq -r '.status' "$JSON_OUTPUT")"
  report_count="$(jq -r '.report_count' "$JSON_OUTPUT")"
  conflict_count="$(jq -r '.conflicts | length' "$JSON_OUTPUT")"
  agreement_rate="$(jq -r '.backend_route_agreement_rate' "$JSON_OUTPUT")"

  [ "$status" = "pass" ] ||
    fail "cohort is not clean: status=$status, reports=$report_count/$MIN_REPORTS, backend_agreement_rate=$agreement_rate/$MIN_BACKEND_AGREEMENT_RATE, conflicts=$conflict_count"
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
