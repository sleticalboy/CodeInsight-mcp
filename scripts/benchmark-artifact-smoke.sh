#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_NAME="codeinsight-benchmark-subset"
EXPECTED_PROFILE="smoke"
EXPECTED_REPOS="p-limit"
OUTPUT_DIR=""
OUTPUT_DIR_WAS_SET=0
REPO_ARG=()
RUN_ID=""
LATEST_SUCCESS_BRANCH=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/benchmark-artifact-smoke.sh [options] <ci-run-id>
       scripts/benchmark-artifact-smoke.sh [options] --latest-success BRANCH

Downloads the CI benchmark subset artifact and validates the Markdown report
with scripts/benchmark-report-smoke.sh plus the compact summary JSON with
scripts/benchmark-summary-text.sh.

Options:
  --repo OWNER/REPO       Pass an explicit GitHub repository to gh.
  --latest-success BRANCH Download from the latest successful CI run on BRANCH.
  --dir PATH             Download into PATH instead of /tmp/codeinsight-benchmark-artifact-<run-id>.
  --artifact-name NAME   Artifact name to download. Default: codeinsight-benchmark-subset.
  --profile PROFILE      Expected benchmark profile. Default: smoke.
  --repos REPO[,REPO]    Expected repository subset. Default: p-limit.
  -h, --help             Show this help.
EOF
  exit "$status"
}

fail() {
  echo "benchmark artifact smoke failed: $*" >&2
  exit 1
}

resolve_latest_success_run() {
  local branch="$1"
  local run_id

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    run_id="$(
      gh run list \
        "${REPO_ARG[@]}" \
        --workflow CI \
        --branch "$branch" \
        --status success \
        --limit 1 \
        --json databaseId \
        --jq '.[0].databaseId // ""'
    )"
  else
    run_id="$(
      gh run list \
        --workflow CI \
        --branch "$branch" \
        --status success \
        --limit 1 \
        --json databaseId \
        --jq '.[0].databaseId // ""'
    )"
  fi

  if [ -z "$run_id" ]; then
    fail "no successful CI run found for branch: $branch"
  fi
  printf "%s" "$run_id"
}

main() {
  local report_file
  local report_count
  local summary_file
  local summary_count

  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h | --help)
        usage 0
        ;;
      --repo)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        REPO_ARG=(--repo "$1")
        ;;
      --latest-success)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        LATEST_SUCCESS_BRANCH="$1"
        ;;
      --dir)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        OUTPUT_DIR="$1"
        OUTPUT_DIR_WAS_SET=1
        ;;
      --artifact-name)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        ARTIFACT_NAME="$1"
        ;;
      --profile)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EXPECTED_PROFILE="$1"
        ;;
      --repos)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EXPECTED_REPOS="$1"
        ;;
      --)
        shift
        break
        ;;
      -*)
        usage
        ;;
      *)
        if [ -n "$RUN_ID" ]; then
          usage
        fi
        RUN_ID="$1"
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    if [ -n "$RUN_ID" ]; then
      usage
    fi
    RUN_ID="$1"
    shift
  done

  if ! command -v gh >/dev/null 2>&1; then
    fail "missing required command: gh"
  fi
  if [ -n "$LATEST_SUCCESS_BRANCH" ] && [ -n "$RUN_ID" ]; then
    usage
  fi
  if [ -z "$RUN_ID" ]; then
    if [ -n "$LATEST_SUCCESS_BRANCH" ]; then
      RUN_ID="$(resolve_latest_success_run "$LATEST_SUCCESS_BRANCH")"
      echo "using latest successful CI run on $LATEST_SUCCESS_BRANCH: $RUN_ID"
    else
      usage
    fi
  fi
  if [ -z "$OUTPUT_DIR" ]; then
    OUTPUT_DIR="${TMPDIR:-/tmp}/codeinsight-benchmark-artifact-$RUN_ID"
  fi

  if [ -e "$OUTPUT_DIR" ]; then
    if [ "$OUTPUT_DIR_WAS_SET" -eq 1 ]; then
      if [ -n "$(find "$OUTPUT_DIR" -mindepth 1 -print -quit)" ]; then
        fail "output directory already exists and is not empty: $OUTPUT_DIR"
      fi
    else
      case "$OUTPUT_DIR" in
        "${TMPDIR:-/tmp}"/codeinsight-benchmark-artifact-*) rm -rf "$OUTPUT_DIR" ;;
        *) fail "refusing to replace unexpected output directory: $OUTPUT_DIR" ;;
      esac
    fi
  fi
  mkdir -p "$OUTPUT_DIR"

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    gh run download "$RUN_ID" \
      "${REPO_ARG[@]}" \
      --name "$ARTIFACT_NAME" \
      --dir "$OUTPUT_DIR"
  else
    gh run download "$RUN_ID" \
      --name "$ARTIFACT_NAME" \
      --dir "$OUTPUT_DIR"
  fi

  report_count="$(find "$OUTPUT_DIR" -type f -name '*.md' | wc -l | tr -d ' ')"
  if [ "$report_count" -ne 1 ]; then
    find "$OUTPUT_DIR" -type f -print >&2
    fail "expected exactly one Markdown report in $OUTPUT_DIR, found $report_count"
  fi
  report_file="$(find "$OUTPUT_DIR" -type f -name '*.md' | head -n 1)"

  summary_count="$(find "$OUTPUT_DIR" -type f -name '*.json' | wc -l | tr -d ' ')"
  if [ "$summary_count" -ne 1 ]; then
    find "$OUTPUT_DIR" -type f -print >&2
    fail "expected exactly one JSON summary in $OUTPUT_DIR, found $summary_count"
  fi
  summary_file="$(find "$OUTPUT_DIR" -type f -name '*.json' | head -n 1)"

  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" \
    "$report_file" \
    "$EXPECTED_PROFILE" \
    "$EXPECTED_REPOS"
  "$ROOT_DIR/scripts/benchmark-summary-text.sh" "$summary_file" >/dev/null

  echo "benchmark artifact smoke passed"
  echo "report: $report_file"
  echo "summary: $summary_file"
}

main "$@"
