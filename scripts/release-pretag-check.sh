#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCHMARK_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/benchmark-artifact-smoke.sh}"
CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/context-pack-quality-artifact-smoke.sh}"
AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/agent-route-artifact-smoke.sh}"
REPO_ARG=()
RUN_ID=""
HEAD_SHA=""
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
subset, context-pack quality, and agent-route artifacts for that exact run. With
--head-sha, resolves a successful CI run for that exact commit so cancelled or
failed stale runs cannot satisfy release evidence. Use before creating a release
tag.

Options:
  --repo OWNER/REPO  Pass an explicit GitHub repository to gh and artifact smoke.
  --run-id ID        Check this CI run instead of resolving the latest branch run.
  --head-sha SHA     Resolve the CI run for this exact commit SHA on the branch.
  -h, --help         Show this help.

Environment:
  CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT=scripts/benchmark-artifact-smoke.sh
  CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT=scripts/context-pack-quality-artifact-smoke.sh
  CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT=scripts/agent-route-artifact-smoke.sh
EOF
  exit "$status"
}

fail() {
  echo "release pretag check failed: $*" >&2
  exit 1
}

resolve_latest_run() {
  local branch="$1"
  local head_sha="$2"
  local run_id
  local jq_filter='.[0].databaseId // ""'
  local limit=1
  local gh_args=()

  if [ -n "$head_sha" ]; then
    jq_filter="map(select(.headSha == \"$head_sha\"))[0].databaseId // \"\""
    limit=20
  fi

  gh_args=(run list)
  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    gh_args+=("${REPO_ARG[@]}")
  fi
  gh_args+=(--workflow CI --branch "$branch")
  if [ -n "$head_sha" ]; then
    gh_args+=(--status success)
  fi
  gh_args+=(--limit "$limit" --json databaseId,headSha --jq "$jq_filter")

  run_id="$(gh "${gh_args[@]}")"

  if [ -z "$run_id" ]; then
    if [ -n "$head_sha" ]; then
      fail "no successful CI run found for branch: $branch and head SHA: $head_sha"
    fi
    fail "no CI run found for branch: $branch"
  fi
  printf "%s" "$run_id"
}

resolve_run_head_sha() {
  local run_id="$1"
  local head_sha
  local gh_args=()

  gh_args=(run view "$run_id")
  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    gh_args+=("${REPO_ARG[@]}")
  fi
  gh_args+=(--json headSha --jq '.headSha // ""')

  head_sha="$(gh "${gh_args[@]}")"
  if [ -z "$head_sha" ]; then
    fail "could not resolve head SHA for CI run: $run_id"
  fi
  printf "%s" "$head_sha"
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
      --head-sha)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        HEAD_SHA="$1"
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
  if [ ! -x "$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "context-pack quality artifact smoke script is not executable: $CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT"
  fi
  if [ ! -x "$AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "agent-route artifact smoke script is not executable: $AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT"
  fi
  if [ -n "$RUN_ID" ] && [ -n "$HEAD_SHA" ]; then
    fail "--run-id and --head-sha cannot be used together"
  fi

  if [ -z "$RUN_ID" ]; then
    RUN_ID="$(resolve_latest_run "$BRANCH" "$HEAD_SHA")"
  fi
  RESOLVED_HEAD_SHA="$HEAD_SHA"
  if [ -z "$RESOLVED_HEAD_SHA" ]; then
    RESOLVED_HEAD_SHA="$(resolve_run_head_sha "$RUN_ID")"
  fi

  echo "watching CI run: $RUN_ID"
  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    gh run watch "$RUN_ID" "${REPO_ARG[@]}" --exit-status
    "$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" "$RUN_ID"
    "$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" "$RUN_ID"
    "$AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" "$RUN_ID"
  else
    gh run watch "$RUN_ID" --exit-status
    "$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" "$RUN_ID"
    "$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" "$RUN_ID"
    "$AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT" "$RUN_ID"
  fi

  echo "release pretag evidence"
  echo "branch: $BRANCH"
  echo "ci_run: $RUN_ID"
  echo "head_sha: $RESOLVED_HEAD_SHA"
  echo "artifact_gate_benchmark: passed"
  echo "artifact_gate_context_pack_quality: passed"
  echo "artifact_gate_agent_route: passed"
  echo "release pretag check passed"
}

main "$@"
