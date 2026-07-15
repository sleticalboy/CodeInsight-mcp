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

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/benchmark-artifact-smoke.sh [options] <ci-run-id>

Downloads the CI benchmark subset artifact and validates the Markdown report
with scripts/benchmark-report-smoke.sh.

Options:
  --repo OWNER/REPO       Pass an explicit GitHub repository to gh.
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

main() {
  local report_file
  local report_count

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

  if [ -z "$RUN_ID" ]; then
    usage
  fi
  if ! command -v gh >/dev/null 2>&1; then
    fail "missing required command: gh"
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

  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" \
    "$report_file" \
    "$EXPECTED_PROFILE" \
    "$EXPECTED_REPOS"

  echo "benchmark artifact smoke passed"
  echo "report: $report_file"
}

main "$@"
