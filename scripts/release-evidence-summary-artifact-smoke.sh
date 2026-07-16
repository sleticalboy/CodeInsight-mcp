#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
RELEASE_EVIDENCE_SUMMARY_SCRIPT="${CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT:-$ROOT_DIR/scripts/release-evidence-summary.sh}"
REPO_ARG=()
REPO=""
RUN_ID=""
BRANCH="main"
TAG_NAME=""
LATEST_SUCCESS_BRANCH=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-evidence-summary-artifact-smoke.sh [options] <ci-run-id>
       scripts/release-evidence-summary-artifact-smoke.sh [options] --latest-success BRANCH

Runs release-evidence-summary against a real completed CI run, validates that
the run head SHA is bound explicitly, and checks release evidence artifact links
plus downloaded report paths.

Options:
  --repo OWNER/REPO       Pass an explicit GitHub repository to gh.
  --latest-success BRANCH Use the latest successful CI run on BRANCH.
  --tag TAG               Release metadata tag. Default: v<Cargo.toml version>.
  -h, --help              Show this help.

Environment:
  CODEINSIGHT_ROOT_DIR=/path/to/repo
  CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT=scripts/release-evidence-summary.sh
EOF
  exit "$status"
}

fail() {
  echo "release evidence summary artifact smoke failed: $*" >&2
  exit 1
}

current_release_tag() {
  ruby - "$ROOT_DIR/Cargo.toml" <<'RUBY'
cargo_path = ARGV.fetch(0)
cargo = File.read(cargo_path)
version = cargo[/^\[package\]\n(?<body>.*?)(?=^\[|\z)/m, :body]&.[](/^version = "([^"]+)"/, 1)
abort("Cargo.toml package version not found") unless version
puts "v#{version}"
RUBY
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

resolve_run_head_sha() {
  local run_id="$1"

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    gh run view "$run_id" "${REPO_ARG[@]}" --json headSha --jq '.headSha'
  else
    gh run view "$run_id" --json headSha --jq '.headSha'
  fi
}

resolve_repo() {
  if [ -n "$REPO" ]; then
    printf "%s" "$REPO"
    return 0
  fi

  gh repo view --json nameWithOwner --jq '.nameWithOwner'
}

require_output() {
  local output_file="$1"
  local pattern="$2"
  local description="$3"

  if ! grep -Fq -- "$pattern" "$output_file"; then
    fail "missing $description: $pattern"
  fi
}

main() {
  local head_sha
  local output_file
  local repo_name

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
        REPO="$1"
        REPO_ARG=(--repo "$1")
        ;;
      --latest-success)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        LATEST_SUCCESS_BRANCH="$1"
        BRANCH="$1"
        ;;
      --tag)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        TAG_NAME="$1"
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
  if [ ! -x "$RELEASE_EVIDENCE_SUMMARY_SCRIPT" ]; then
    fail "release evidence summary script is not executable: $RELEASE_EVIDENCE_SUMMARY_SCRIPT"
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
  if [ -z "$TAG_NAME" ]; then
    TAG_NAME="$(current_release_tag)"
  fi

  head_sha="$(resolve_run_head_sha "$RUN_ID")"
  if [ -z "$head_sha" ]; then
    fail "could not resolve head SHA for CI run: $RUN_ID"
  fi
  repo_name="$(resolve_repo)"
  output_file="$(mktemp)"

  CODEINSIGHT_ROOT_DIR="$ROOT_DIR" \
    "$RELEASE_EVIDENCE_SUMMARY_SCRIPT" \
      "${REPO_ARG[@]}" \
      --run-id "$RUN_ID" \
      --head-sha "$head_sha" \
      "$TAG_NAME" \
      "$BRANCH" >"$output_file"

  require_output "$output_file" "ci_run: $RUN_ID" "CI run line"
  require_output "$output_file" "head_sha: $head_sha" "head SHA line"
  require_output "$output_file" "benchmark_artifact_url: https://github.com/$repo_name/actions/runs/$RUN_ID/artifacts/" "benchmark artifact URL"
  require_output "$output_file" "context_pack_quality_artifact_url: https://github.com/$repo_name/actions/runs/$RUN_ID/artifacts/" "context-pack quality artifact URL"
  require_output "$output_file" "agent_route_artifact_url: https://github.com/$repo_name/actions/runs/$RUN_ID/artifacts/" "agent-route artifact URL"
  require_output "$output_file" "benchmark_report: " "benchmark report path"
  require_output "$output_file" "context_pack_quality_summary: " "context-pack quality summary path"
  require_output "$output_file" "agent_route_summary: " "agent-route summary path"
  require_output "$output_file" "- Context-pack quality artifact: [codeinsight-context-pack-quality]" "release notes context-pack quality link"
  require_output "$output_file" "- Agent-route artifact: [codeinsight-agent-route-smoke]" "release notes agent-route link"

  cat "$output_file"
  rm -f "$output_file"
  echo "release evidence summary artifact smoke passed"
}

main "$@"
