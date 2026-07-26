#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST=""
OUTPUT_DIR="/tmp/codeinsight-codebase-memory-bridge-cohort"
MIN_REPORTS=1
MIN_BACKEND_AGREEMENT_RATE=100
CHECK=false

fail() {
  echo "codebase-memory bridge cohort report failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

usage() {
  cat <<'EOF'
usage: scripts/codebase-memory-bridge-cohort-report.sh --manifest PATH [options]

Generate per-task codebase-memory bridge reports and aggregate them into a
cohort summary.

Manifest format:
  slug<TAB>task<TAB>backend_evidence_json<TAB>agent_route_json

Blank lines and lines starting with "#" are ignored. A header row starting with
"slug<TAB>task<TAB>" is also ignored.

Options:
  --manifest PATH    TSV manifest with one row per bridge task.
  --output-dir DIR   Output directory. Default: /tmp/codeinsight-codebase-memory-bridge-cohort.
  --min-reports N    Minimum report count for --check. Default: 1.
  --min-backend-agreement-rate N
                    Minimum percent of reports whose backend_route_agreement.status is agree.
                    Default: 100.
  --check            Fail unless the aggregated cohort is clean.
  -h, --help         Show this help text.

Output:
  <output-dir>/reports/<slug>/summary.json
  <output-dir>/reports/<slug>/codebase-memory-bridge-report.md
  <output-dir>/cohort.md
  <output-dir>/cohort.json
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --manifest)
        [ "$#" -ge 2 ] || fail "--manifest requires a path"
        MANIFEST="$2"
        shift 2
        ;;
      --output-dir)
        [ "$#" -ge 2 ] || fail "--output-dir requires a path"
        OUTPUT_DIR="$2"
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
        fail "unknown argument: $1"
        ;;
    esac
  done
}

validate_args() {
  require_command jq

  [ -n "$MANIFEST" ] || fail "--manifest is required"
  [ -f "$MANIFEST" ] || fail "manifest does not exist: $MANIFEST"

  case "$MIN_REPORTS" in
    ''|*[!0-9]*) fail "--min-reports must be a positive integer" ;;
  esac
  [ "$MIN_REPORTS" -gt 0 ] || fail "--min-reports must be > 0"
  case "$MIN_BACKEND_AGREEMENT_RATE" in
    ''|*[!0-9]*) fail "--min-backend-agreement-rate must be an integer from 0 to 100" ;;
  esac
  [ "$MIN_BACKEND_AGREEMENT_RATE" -le 100 ] ||
    fail "--min-backend-agreement-rate must be <= 100"
}

is_ignored_manifest_line() {
  local slug="$1"
  local task="$2"

  [ -z "$slug$task" ] && return 0
  case "$slug" in
    \#*) return 0 ;;
  esac
  [ "$slug" = "slug" ] && [ "$task" = "task" ] && return 0
  return 1
}

safe_slug() {
  local slug="$1"

  slug="${slug// /-}"
  slug="$(printf '%s' "$slug" | tr -c 'A-Za-z0-9._-' '-')"
  slug="${slug##-}"
  slug="${slug%%-}"
  [ -n "$slug" ] || fail "manifest row produced an empty slug"
  printf '%s\n' "$slug"
}

run_reports() {
  local row=0
  local reports=()
  local slug task backend_evidence agent_route extra report_dir

  mkdir -p "$OUTPUT_DIR/reports"

  while IFS=$'\t' read -r slug task backend_evidence agent_route extra || [ -n "${slug:-}" ]; do
    row=$((row + 1))
    if is_ignored_manifest_line "${slug:-}" "${task:-}"; then
      continue
    fi

    [ -z "${extra:-}" ] || fail "manifest row $row has too many columns"
    [ -n "${slug:-}" ] || fail "manifest row $row missing slug"
    [ -n "${task:-}" ] || fail "manifest row $row missing task"
    [ -n "${backend_evidence:-}" ] || fail "manifest row $row missing backend evidence path"
    [ -n "${agent_route:-}" ] || fail "manifest row $row missing agent-route JSON path"
    [ -f "$backend_evidence" ] || fail "manifest row $row backend evidence does not exist: $backend_evidence"
    [ -f "$agent_route" ] || fail "manifest row $row agent-route JSON does not exist: $agent_route"

    slug="$(safe_slug "$slug")"
    report_dir="$OUTPUT_DIR/reports/$slug"
    "$ROOT_DIR/scripts/codebase-memory-bridge-report.sh" \
      --backend-evidence "$backend_evidence" \
      --agent-route-json "$agent_route" \
      --task "$task" \
      --output-dir "$report_dir" >/dev/null
    reports+=("$report_dir")
  done <"$MANIFEST"

  [ "${#reports[@]}" -gt 0 ] || fail "manifest produced no reports"

  local check_args=()
  if [ "$CHECK" = true ]; then
    check_args+=(--check)
  fi

  "$ROOT_DIR/scripts/codebase-memory-bridge-cohort-summary.sh" \
    "${reports[@]}" \
    --min-reports "$MIN_REPORTS" \
    --min-backend-agreement-rate "$MIN_BACKEND_AGREEMENT_RATE" \
    --output "$OUTPUT_DIR/cohort.md" \
    --json "$OUTPUT_DIR/cohort.json" \
    "${check_args[@]}"
}

main() {
  parse_args "$@"
  validate_args
  run_reports

  echo "codebase-memory bridge cohort report written to $OUTPUT_DIR/cohort.md"
  echo "summary: $OUTPUT_DIR/cohort.json"
  echo "reports: $OUTPUT_DIR/reports"
}

main "$@"
