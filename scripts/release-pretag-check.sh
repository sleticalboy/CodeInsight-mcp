#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCHMARK_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/benchmark-artifact-smoke.sh}"
REPO_ARG=()
RUN_ID=""
BRANCH="main"

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-pretag-check.sh [options] [branch]

Waits for the latest CI run on a branch, then validates the uploaded benchmark
subset artifact for that exact run. Use before creating a release tag.

Options:
  --repo OWNER/REPO  Pass an explicit GitHub repository to gh and artifact smoke.
  --run-id ID        Check this CI run instead of resolving the latest branch run.
  -h, --help         Show this help.

Environment:
  CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT=scripts/benchmark-artifact-smoke.sh
EOF
  exit "$status"
}

fail() {
  echo "release pretag check failed: $*" >&2
  exit 1
}

resolve_latest_run() {
  local branch="$1"
  local run_id

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    run_id="$(
      gh run list \
        "${REPO_ARG[@]}" \
        --workflow CI \
        --branch "$branch" \
        --limit 1 \
        --json databaseId \
        --jq '.[0].databaseId // ""'
    )"
  else
    run_id="$(
      gh run list \
        --workflow CI \
        --branch "$branch" \
        --limit 1 \
        --json databaseId \
        --jq '.[0].databaseId // ""'
    )"
  fi

  if [ -z "$run_id" ]; then
    fail "no CI run found for branch: $branch"
  fi
  printf "%s" "$run_id"
}

main() {
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
      --run-id)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        RUN_ID="$1"
        ;;
      --)
        shift
        break
        ;;
      -*)
        usage
        ;;
      *)
        if [ -n "$BRANCH" ] && [ "$BRANCH" != "main" ]; then
          usage
        fi
        BRANCH="$1"
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    if [ -n "$BRANCH" ] && [ "$BRANCH" != "main" ]; then
      usage
    fi
    BRANCH="$1"
    shift
  done

  if ! command -v gh >/dev/null 2>&1; then
    fail "missing required command: gh"
  fi
  if [ ! -x "$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "benchmark artifact smoke script is not executable: $BENCHMARK_ARTIFACT_SMOKE_SCRIPT"
  fi

  if [ -z "$RUN_ID" ]; then
    RUN_ID="$(resolve_latest_run "$BRANCH")"
  fi

  echo "watching CI run: $RUN_ID"
  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    gh run watch "$RUN_ID" "${REPO_ARG[@]}" --exit-status
    "$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" "$RUN_ID"
  else
    gh run watch "$RUN_ID" --exit-status
    "$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" "$RUN_ID"
  fi

  echo "release pretag check passed"
}

main "$@"
