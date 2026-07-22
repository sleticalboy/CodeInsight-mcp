#!/usr/bin/env bash
set -euo pipefail

ARTIFACT_NAME="codeinsight-context-pack-quality"
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
usage: scripts/context-pack-quality-artifact-smoke.sh [options] <ci-run-id>
       scripts/context-pack-quality-artifact-smoke.sh [options] --latest-success BRANCH

Downloads the CI context-pack quality artifact and validates the JSON summary.

Options:
  --repo OWNER/REPO       Pass an explicit GitHub repository to gh.
  --latest-success BRANCH Download from the latest successful CI run on BRANCH.
  --dir PATH             Download into PATH instead of /tmp/codeinsight-context-pack-quality-artifact-<run-id>.
  --artifact-name NAME   Artifact name to download. Default: codeinsight-context-pack-quality.
  -h, --help             Show this help.
EOF
  exit "$status"
}

fail() {
  echo "context-pack quality artifact smoke failed: $*" >&2
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

validate_summary_json() {
  local summary_file="$1"

  if ! jq -e \
    '.status == "pass"
      and .scenarios_passed == 10
      and (.scenarios | length) == 10
      and all(.scenarios[]; .status == "pass")
      and (.scenarios[] | select(.name == "polyglot_symbol_web_controller" and .metrics.first_file == "src/app.ts"))
      and (.scenarios[] | select(.name == "polyglot_symbol_web_controller" and (.metrics.first_reading_focus | type == "string" and length > 0)))
      and (.scenarios[] | select(.name == "polyglot_symbol_web_controller" and (.metrics.first_reading_question | type == "string" and length > 0)))
      and (.scenarios[] | select(.name == "polyglot_symbol_php_service_render" and .metrics.first_file == "src/PhpService.php"))
      and (.scenarios[] | select(.name == "dependency_continuation" and .metrics.dependency_file == "app/support.py"))
      and (.scenarios[] | select(.name == "budget_continuation" and .metrics.omitted_candidates > 0 and .metrics.continuation_status == "omitted_candidates_available"))
      and (.scenarios[] | select(.name == "minimum_budget" and .metrics.applied_token_budget == 500 and .metrics.continuation_status == "minimum_budget_applied"))
      and (.scenarios[] | select(.name == "token_exhaustion" and .metrics.truncated == true and .metrics.continuation_status == "token_budget_exhausted"))
      and (.scenarios[] | select(.name == "core_analysis_question_coverage"))
      and .question_checks_passed == 11
      and (.question_checks | length) == 11
      and all(.question_checks[]; .status == "pass" and (.focus | type == "string" and length > 0) and (.question | type == "string" and length > 0))' \
    "$summary_file" >/dev/null; then
    fail "$summary_file does not match expected context-pack quality metrics"
  fi
}

main() {
  local summary_count
  local summary_file

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
  if ! command -v jq >/dev/null 2>&1; then
    fail "missing required command: jq"
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
    OUTPUT_DIR="${TMPDIR:-/tmp}/codeinsight-context-pack-quality-artifact-$RUN_ID"
  fi

  if [ -e "$OUTPUT_DIR" ]; then
    if [ "$OUTPUT_DIR_WAS_SET" -eq 1 ]; then
      if [ -n "$(find "$OUTPUT_DIR" -mindepth 1 -print -quit)" ]; then
        fail "output directory already exists and is not empty: $OUTPUT_DIR"
      fi
    else
      case "$OUTPUT_DIR" in
        "${TMPDIR:-/tmp}"/codeinsight-context-pack-quality-artifact-*) rm -rf "$OUTPUT_DIR" ;;
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

  summary_count="$(find "$OUTPUT_DIR" -type f -name '*.json' | wc -l | tr -d ' ')"
  if [ "$summary_count" -ne 1 ]; then
    find "$OUTPUT_DIR" -type f -print >&2
    fail "expected exactly one JSON summary in $OUTPUT_DIR, found $summary_count"
  fi
  summary_file="$(find "$OUTPUT_DIR" -type f -name '*.json' | head -n 1)"

  validate_summary_json "$summary_file"

  echo "context-pack quality artifact smoke passed"
  echo "summary: $summary_file"
  echo "first_reading_focus: $(jq -r '.scenarios[] | select(.name == "polyglot_symbol_web_controller") | .metrics.first_reading_focus' "$summary_file")"
  echo "first_reading_question: $(jq -r '.scenarios[] | select(.name == "polyglot_symbol_web_controller") | .metrics.first_reading_question' "$summary_file")"
  echo "question_checks_passed: $(jq -r '.question_checks_passed' "$summary_file")"
}

main "$@"
