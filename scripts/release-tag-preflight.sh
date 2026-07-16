#!/usr/bin/env bash
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$SCRIPT_ROOT}"
RELEASE_WORKFLOW_GUARD_SCRIPT="${CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT:-$ROOT_DIR/scripts/release-workflow-guard-smoke.sh}"
RELEASE_PRETAG_CHECK_SCRIPT="${CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT:-$ROOT_DIR/scripts/release-pretag-check.sh}"
RELEASE_METADATA_SUMMARY_SCRIPT="${CODEINSIGHT_RELEASE_METADATA_SUMMARY_SCRIPT:-$SCRIPT_ROOT/scripts/release-metadata-summary.sh}"
REPO_ARG=()
REPO=""
BRANCH="main"
HEAD_SHA=""
TAG_NAME=""
METADATA_SUMMARY=""
PRETAG_OUTPUT_FILE=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-tag-preflight.sh [options] <tag> [branch]

Dry-run the release tag preflight without creating or pushing a tag. This checks
that the tagged Release Build workflow is guarded correctly and that the tag
target commit already has successful CI plus valid benchmark subset,
context-pack quality, and agent-route artifacts.

Options:
  --repo OWNER/REPO  Pass an explicit GitHub repository to pretag validation.
  --head-sha SHA     Check this commit instead of the current HEAD.
  -h, --help         Show this help.

Environment:
  CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT=scripts/release-workflow-guard-smoke.sh
  CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT=scripts/release-pretag-check.sh
  CODEINSIGHT_RELEASE_METADATA_SUMMARY_SCRIPT=scripts/release-metadata-summary.sh
  CODEINSIGHT_ROOT_DIR=/path/to/repo
EOF
  exit "$status"
}

fail() {
  echo "release tag preflight failed: $*" >&2
  exit 1
}

cleanup() {
  if [ -n "$PRETAG_OUTPUT_FILE" ]; then
    rm -f "$PRETAG_OUTPUT_FILE"
  fi
}

require_pretag_evidence() {
  local output="$1"
  local literal="$2"
  local description="$3"

  if ! grep -Fq -- "$literal" <<<"$output"; then
    fail "release pretag evidence is missing ${description}: $literal"
  fi
}

gh_checked_release_missing() {
  local tag="$1"
  local output status

  if [ -n "$REPO" ]; then
    set +e
    output="$(gh release view "$tag" --repo "$REPO" --json tagName --jq '.tagName' 2>&1)"
    status="$?"
    set -e
  else
    set +e
    output="$(gh release view "$tag" --json tagName --jq '.tagName' 2>&1)"
    status="$?"
    set -e
  fi

  if [ "$status" -eq 0 ]; then
    fail "remote GitHub Release already exists: $tag"
  fi

  case "$output" in
    *"release not found"* | *"Release not found"* | *"Not Found"* | *"HTTP 404"*)
      return 0
      ;;
    *)
      fail "could not check remote GitHub Release for $tag: $output"
      ;;
  esac
}

remote_tag_missing() {
  local tag="$1"
  local remote status

  if [ -n "$REPO" ]; then
    remote="https://github.com/${REPO}.git"
  else
    remote="$(git -C "$ROOT_DIR" remote get-url origin)"
  fi

  set +e
  git ls-remote --exit-code --tags "$remote" "refs/tags/$tag" >/dev/null 2>&1
  status="$?"
  set -e

  case "$status" in
    0)
      fail "remote tag already exists: $tag"
      ;;
    2)
      return 0
      ;;
    *)
      fail "could not check remote tag $tag from $remote"
      ;;
  esac
}

normalize_tag() {
  local tag="$1"
  case "$tag" in
    v*) printf "%s" "$tag" ;;
    *) printf "v%s" "$tag" ;;
  esac
}

main() {
  trap cleanup EXIT INT TERM

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

  if [ -z "$HEAD_SHA" ]; then
    HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"
  fi

  if git -C "$ROOT_DIR" rev-parse -q --verify "refs/tags/$TAG_NAME" >/dev/null; then
    fail "local tag already exists: $TAG_NAME"
  fi
  if [ ! -x "$RELEASE_METADATA_SUMMARY_SCRIPT" ]; then
    fail "release metadata summary script is not executable: $RELEASE_METADATA_SUMMARY_SCRIPT"
  fi
  METADATA_SUMMARY="$(
    CODEINSIGHT_ROOT_DIR="$ROOT_DIR" \
      CODEINSIGHT_RELEASE_METADATA_CONTEXT="release tag preflight" \
      "$RELEASE_METADATA_SUMMARY_SCRIPT" "$TAG_NAME"
  )"
  if ! command -v gh >/dev/null 2>&1; then
    fail "missing required command: gh"
  fi
  remote_tag_missing "$TAG_NAME"
  gh_checked_release_missing "$TAG_NAME"
  if [ ! -x "$RELEASE_WORKFLOW_GUARD_SCRIPT" ]; then
    fail "release workflow guard script is not executable: $RELEASE_WORKFLOW_GUARD_SCRIPT"
  fi
  if [ ! -x "$RELEASE_PRETAG_CHECK_SCRIPT" ]; then
    fail "release pretag check script is not executable: $RELEASE_PRETAG_CHECK_SCRIPT"
  fi

  echo "release tag preflight"
  echo "tag: $TAG_NAME"
  echo "branch: $BRANCH"
  echo "head_sha: $HEAD_SHA"
  printf "%s\n" "$METADATA_SUMMARY"

  "$RELEASE_WORKFLOW_GUARD_SCRIPT"
  PRETAG_OUTPUT_FILE="$(mktemp "${TMPDIR:-/tmp}/codeinsight-pretag-output.XXXXXX")"
  "$RELEASE_PRETAG_CHECK_SCRIPT" "${REPO_ARG[@]}" --head-sha "$HEAD_SHA" "$BRANCH" | tee "$PRETAG_OUTPUT_FILE"
  PRETAG_OUTPUT="$(cat "$PRETAG_OUTPUT_FILE")"
  require_pretag_evidence "$PRETAG_OUTPUT" "release pretag evidence" "evidence heading"
  require_pretag_evidence "$PRETAG_OUTPUT" "branch: $BRANCH" "branch"
  require_pretag_evidence "$PRETAG_OUTPUT" "ci_run:" "CI run"
  require_pretag_evidence "$PRETAG_OUTPUT" "head_sha: $HEAD_SHA" "head SHA"
  require_pretag_evidence "$PRETAG_OUTPUT" "artifact_gate_benchmark: passed" "benchmark artifact gate"
  require_pretag_evidence "$PRETAG_OUTPUT" "artifact_gate_context_pack_quality: passed" "context-pack quality artifact gate"
  require_pretag_evidence "$PRETAG_OUTPUT" "artifact_gate_agent_route: passed" "agent-route artifact gate"

  echo "release tag preflight passed"
  echo "next: git tag -a $TAG_NAME -m \"$TAG_NAME\" && git push origin $TAG_NAME"
}

main "$@"
