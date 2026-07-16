#!/usr/bin/env bash
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$SCRIPT_ROOT}"
BENCHMARK_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/benchmark-artifact-smoke.sh}"
CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/context-pack-quality-artifact-smoke.sh}"
RELEASE_METADATA_SUMMARY_SCRIPT="${CODEINSIGHT_RELEASE_METADATA_SUMMARY_SCRIPT:-$SCRIPT_ROOT/scripts/release-metadata-summary.sh}"
ARTIFACT_NAME="codeinsight-benchmark-subset"
CONTEXT_PACK_QUALITY_ARTIFACT_NAME="codeinsight-context-pack-quality"
REPO_ARG=()
REPO=""
BRANCH="main"
HEAD_SHA=""
RUN_ID=""
TAG_NAME=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-evidence-summary.sh [options] <tag> [branch]

Build a copyable pre-release evidence summary for the target tag. The script
verifies release metadata, resolves the successful CI run for the tag target
SHA, validates the benchmark subset and context-pack quality artifacts, and
prints a Markdown block for release notes or handoff checklists.

Options:
  --repo OWNER/REPO       Pass an explicit GitHub repository to gh.
  --head-sha SHA          Check this commit instead of the current HEAD.
  --run-id ID             Use this CI run instead of resolving by head SHA.
  --artifact-name NAME    Benchmark artifact name. Default: codeinsight-benchmark-subset.
  --quality-artifact-name NAME
                          Context-pack quality artifact name. Default: codeinsight-context-pack-quality.
  -h, --help              Show this help.

Environment:
  CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT=scripts/benchmark-artifact-smoke.sh
  CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT=scripts/context-pack-quality-artifact-smoke.sh
  CODEINSIGHT_RELEASE_METADATA_SUMMARY_SCRIPT=scripts/release-metadata-summary.sh
  CODEINSIGHT_ROOT_DIR=/path/to/repo
EOF
  exit "$status"
}

fail() {
  echo "release evidence summary failed: $*" >&2
  exit 1
}

normalize_tag() {
  local tag="$1"
  case "$tag" in
    v*) printf "%s" "$tag" ;;
    *) printf "v%s" "$tag" ;;
  esac
}

resolve_repo() {
  if [ -n "$REPO" ]; then
    printf "%s" "$REPO"
    return 0
  fi

  gh repo view --json nameWithOwner --jq '.nameWithOwner'
}

resolve_run_by_head_sha() {
  local branch="$1"
  local head_sha="$2"
  local run_id

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    run_id="$(
      gh run list \
        "${REPO_ARG[@]}" \
        --workflow CI \
        --branch "$branch" \
        --status success \
        --limit 20 \
        --json databaseId,headSha \
        --jq "map(select(.headSha == \"$head_sha\"))[0].databaseId // \"\""
    )"
  else
    run_id="$(
      gh run list \
        --workflow CI \
        --branch "$branch" \
        --status success \
        --limit 20 \
        --json databaseId,headSha \
        --jq "map(select(.headSha == \"$head_sha\"))[0].databaseId // \"\""
    )"
  fi

  if [ -z "$run_id" ]; then
    fail "no successful CI run found for branch: $branch and head SHA: $head_sha"
  fi
  printf "%s" "$run_id"
}

validate_run() {
  local run_id="$1"
  local expected_head_sha="$2"
  local run_json

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    run_json="$(gh run view "$run_id" "${REPO_ARG[@]}" --json conclusion,databaseId,headSha,status,url)"
  else
    run_json="$(gh run view "$run_id" --json conclusion,databaseId,headSha,status,url)"
  fi

  RUN_JSON="$run_json" ruby -rjson - "$expected_head_sha" <<'RUBY'
expected_head_sha = ARGV.fetch(0)
run = JSON.parse(ENV.fetch("RUN_JSON"))

def fail!(message)
  warn("release evidence summary failed: #{message}")
  exit(1)
end

fail!("CI run is not completed: #{run["status"]}") unless run["status"] == "completed"
fail!("CI run did not succeed: #{run["conclusion"]}") unless run["conclusion"] == "success"
fail!("CI run head SHA #{run["headSha"]} does not match #{expected_head_sha}") unless run["headSha"] == expected_head_sha

puts "ci_run: #{run["databaseId"]}"
puts "ci_url: #{run["url"]}"
RUBY
}

resolve_artifact_url() {
  local repo="$1"
  local run_id="$2"
  local artifact_name="$3"
  local artifact_id

  artifact_id="$(
    gh api "repos/$repo/actions/runs/$run_id/artifacts" \
      --jq ".artifacts[] | select(.name == \"$artifact_name\") | .id" \
      | head -n 1
  )"

  if [ -z "$artifact_id" ]; then
    fail "artifact not found on CI run $run_id: $artifact_name"
  fi

  printf "https://github.com/%s/actions/runs/%s/artifacts/%s" "$repo" "$run_id" "$artifact_id"
}

validate_benchmark_artifact() {
  local run_id="$1"
  local output

  if [ ! -x "$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "benchmark artifact smoke script is not executable: $BENCHMARK_ARTIFACT_SMOKE_SCRIPT"
  fi

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    output="$("$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" --artifact-name "$ARTIFACT_NAME" "$run_id")"
  else
    output="$("$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" --artifact-name "$ARTIFACT_NAME" "$run_id")"
  fi

  printf "%s\n" "$output" | awk -F': ' '/^report: / { print $2; exit }'
}

validate_context_pack_quality_artifact() {
  local run_id="$1"
  local output

  if [ ! -x "$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "context-pack quality artifact smoke script is not executable: $CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT"
  fi

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    output="$("$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" --artifact-name "$CONTEXT_PACK_QUALITY_ARTIFACT_NAME" "$run_id")"
  else
    output="$("$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" --artifact-name "$CONTEXT_PACK_QUALITY_ARTIFACT_NAME" "$run_id")"
  fi

  printf "%s\n" "$output" | awk -F': ' '/^summary: / { print $2; exit }'
}

main() {
  local metadata_summary
  local run_summary
  local repo_name
  local ci_url
  local benchmark_artifact_url
  local benchmark_report_file
  local context_pack_quality_artifact_url
  local context_pack_quality_summary_file

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
      --head-sha)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        HEAD_SHA="$1"
        ;;
      --run-id)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        RUN_ID="$1"
        ;;
      --artifact-name)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        ARTIFACT_NAME="$1"
        ;;
      --quality-artifact-name)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        CONTEXT_PACK_QUALITY_ARTIFACT_NAME="$1"
        ;;
      --)
        shift
        break
        ;;
      -*)
        usage
        ;;
      *)
        if [ -z "$TAG_NAME" ]; then
          TAG_NAME="$(normalize_tag "$1")"
        elif [ "$BRANCH" = "main" ]; then
          BRANCH="$1"
        else
          usage
        fi
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    if [ -z "$TAG_NAME" ]; then
      TAG_NAME="$(normalize_tag "$1")"
    elif [ "$BRANCH" = "main" ]; then
      BRANCH="$1"
    else
      usage
    fi
    shift
  done

  if [ -z "$TAG_NAME" ]; then
    usage
  fi
  if [[ ! "$TAG_NAME" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fail "tag must look like vX.Y.Z or X.Y.Z: $TAG_NAME"
  fi
  if [ -n "$RUN_ID" ] && [ -n "$HEAD_SHA" ]; then
    fail "--run-id and --head-sha cannot be used together"
  fi
  if ! command -v gh >/dev/null 2>&1; then
    fail "missing required command: gh"
  fi
  if [ ! -x "$RELEASE_METADATA_SUMMARY_SCRIPT" ]; then
    fail "release metadata summary script is not executable: $RELEASE_METADATA_SUMMARY_SCRIPT"
  fi

  if [ -z "$HEAD_SHA" ]; then
    HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"
  fi

  metadata_summary="$(
    CODEINSIGHT_ROOT_DIR="$ROOT_DIR" \
      CODEINSIGHT_RELEASE_METADATA_CONTEXT="release evidence summary" \
      "$RELEASE_METADATA_SUMMARY_SCRIPT" "$TAG_NAME"
  )"
  if [ -z "$RUN_ID" ]; then
    RUN_ID="$(resolve_run_by_head_sha "$BRANCH" "$HEAD_SHA")"
  fi
  repo_name="$(resolve_repo)"
  run_summary="$(validate_run "$RUN_ID" "$HEAD_SHA")"
  ci_url="$(printf "%s\n" "$run_summary" | awk -F': ' '/^ci_url: / { print $2; exit }')"
  benchmark_artifact_url="$(resolve_artifact_url "$repo_name" "$RUN_ID" "$ARTIFACT_NAME")"
  context_pack_quality_artifact_url="$(resolve_artifact_url "$repo_name" "$RUN_ID" "$CONTEXT_PACK_QUALITY_ARTIFACT_NAME")"
  benchmark_report_file="$(validate_benchmark_artifact "$RUN_ID")"
  context_pack_quality_summary_file="$(validate_context_pack_quality_artifact "$RUN_ID")"

  echo "release evidence summary"
  echo "tag: $TAG_NAME"
  echo "branch: $BRANCH"
  echo "head_sha: $HEAD_SHA"
  printf "%s\n" "$metadata_summary"
  printf "%s\n" "$run_summary"
  echo "benchmark_artifact: $ARTIFACT_NAME"
  echo "benchmark_artifact_url: $benchmark_artifact_url"
  echo "benchmark_report: $benchmark_report_file"
  echo "context_pack_quality_artifact: $CONTEXT_PACK_QUALITY_ARTIFACT_NAME"
  echo "context_pack_quality_artifact_url: $context_pack_quality_artifact_url"
  echo "context_pack_quality_summary: $context_pack_quality_summary_file"
  echo
  echo "release_notes_block:"
  echo "## $TAG_NAME release evidence"
  echo
  echo "- Target commit: \`$HEAD_SHA\`"
  echo "- CI: [run $RUN_ID]($ci_url)"
  echo "- Benchmark artifact: [$ARTIFACT_NAME]($benchmark_artifact_url)"
  echo "- Benchmark report: \`$benchmark_report_file\`"
  echo "- Context-pack quality artifact: [$CONTEXT_PACK_QUALITY_ARTIFACT_NAME]($context_pack_quality_artifact_url)"
  echo "- Context-pack quality summary: \`$context_pack_quality_summary_file\`"
  printf "%s\n" "$metadata_summary" | sed 's/^/- /'
}

main "$@"
