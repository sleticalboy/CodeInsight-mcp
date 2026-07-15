#!/usr/bin/env bash
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$SCRIPT_ROOT}"
PREPARE_RELEASE_SCRIPT="${CODEINSIGHT_PREPARE_RELEASE_SCRIPT:-$SCRIPT_ROOT/scripts/prepare-release.sh}"
RELEASE_TAG_PREFLIGHT_SCRIPT="${CODEINSIGHT_RELEASE_TAG_PREFLIGHT_SCRIPT:-$SCRIPT_ROOT/scripts/release-tag-preflight.sh}"
RELEASE_EVIDENCE_SUMMARY_SCRIPT="${CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT:-$SCRIPT_ROOT/scripts/release-evidence-summary.sh}"
RELEASE_WORKFLOW_GUARD_SCRIPT="${CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT:-$SCRIPT_ROOT/scripts/release-workflow-guard-smoke.sh}"
RELEASE_PRETAG_CHECK_SCRIPT="${CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT:-$SCRIPT_ROOT/scripts/release-pretag-check.sh}"
BENCHMARK_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT:-$SCRIPT_ROOT/scripts/benchmark-artifact-smoke.sh}"
REPO_ARG=()
REPO=""
BRANCH="main"
HEAD_SHA=""
TAG_NAME=""
TEMP_DIR=""
KEEP_TEMP=0

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-dry-run.sh [options] <tag> [branch]

Runs the release prep preview, tag preflight, and evidence summary without
modifying the checkout. The preflight and evidence steps use a temporary copy
of the prepared release metadata, so they can validate the full release path
before a release prep commit exists.

Options:
  --repo OWNER/REPO  Pass an explicit GitHub repository to preflight/evidence.
  --head-sha SHA     Check this commit instead of the current HEAD.
  --keep-temp        Keep the temporary prepared metadata copy for inspection.
  -h, --help         Show this help.

Environment:
  CODEINSIGHT_ROOT_DIR=/path/to/repo
  CODEINSIGHT_RELEASE_DATE=YYYY-MM-DD
  CODEINSIGHT_ALLOW_EMPTY_CHANGELOG=1
  CODEINSIGHT_PREPARE_RELEASE_SCRIPT=scripts/prepare-release.sh
  CODEINSIGHT_RELEASE_TAG_PREFLIGHT_SCRIPT=scripts/release-tag-preflight.sh
  CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT=scripts/release-evidence-summary.sh
EOF
  exit "$status"
}

fail() {
  echo "release dry run failed: $*" >&2
  exit 1
}

cleanup() {
  if [ "$KEEP_TEMP" -eq 0 ] && [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

normalize_tag() {
  local tag="$1"
  case "$tag" in
    v*) printf "%s" "$tag" ;;
    *) printf "v%s" "$tag" ;;
  esac
}

copy_release_metadata_workspace() {
  local target="$1"

  mkdir -p "$target/docs"
  cp "$ROOT_DIR/Cargo.toml" "$ROOT_DIR/Cargo.lock" "$ROOT_DIR/README.md" "$ROOT_DIR/CHANGELOG.md" "$target/"
  cp "$ROOT_DIR/docs/install.md" "$target/docs/install.md"
  git -C "$target" init -q
}

require_executable() {
  local path="$1"
  local description="$2"

  if [ ! -x "$path" ]; then
    fail "$description is not executable: $path"
  fi
}

main() {
  local temp_repo

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
      --keep-temp)
        KEEP_TEMP=1
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

  require_executable "$PREPARE_RELEASE_SCRIPT" "prepare release script"
  require_executable "$RELEASE_TAG_PREFLIGHT_SCRIPT" "release tag preflight script"
  require_executable "$RELEASE_EVIDENCE_SUMMARY_SCRIPT" "release evidence summary script"
  require_executable "$RELEASE_WORKFLOW_GUARD_SCRIPT" "release workflow guard script"
  require_executable "$RELEASE_PRETAG_CHECK_SCRIPT" "release pretag check script"
  require_executable "$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" "benchmark artifact smoke script"

  if [ -z "$HEAD_SHA" ]; then
    HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"
  fi

  TEMP_DIR="$(mktemp -d)"
  temp_repo="$TEMP_DIR/release-metadata"
  copy_release_metadata_workspace "$temp_repo"

  echo "release dry run"
  echo "tag: $TAG_NAME"
  echo "branch: $BRANCH"
  echo "head_sha: $HEAD_SHA"
  if [ "$KEEP_TEMP" -eq 1 ]; then
    echo "temp_metadata: $temp_repo"
  fi
  echo

  echo "[1/4] prepare release diff"
  CODEINSIGHT_ROOT_DIR="$ROOT_DIR" "$PREPARE_RELEASE_SCRIPT" --dry-run "$TAG_NAME"
  echo

  echo "[2/4] prepare temporary release metadata"
  CODEINSIGHT_ROOT_DIR="$temp_repo" \
    CODEINSIGHT_SKIP_CARGO_CHECK=1 \
    "$PREPARE_RELEASE_SCRIPT" "$TAG_NAME"
  echo

  echo "[3/4] release tag preflight"
  CODEINSIGHT_ROOT_DIR="$temp_repo" \
    CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT="$RELEASE_WORKFLOW_GUARD_SCRIPT" \
    CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT="$RELEASE_PRETAG_CHECK_SCRIPT" \
    "$RELEASE_TAG_PREFLIGHT_SCRIPT" "${REPO_ARG[@]}" --head-sha "$HEAD_SHA" "$TAG_NAME" "$BRANCH"
  echo

  echo "[4/4] release evidence summary"
  CODEINSIGHT_ROOT_DIR="$temp_repo" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" \
    "$RELEASE_EVIDENCE_SUMMARY_SCRIPT" "${REPO_ARG[@]}" --head-sha "$HEAD_SHA" "$TAG_NAME" "$BRANCH"
  echo

  echo "release dry run passed"
  echo "next: scripts/prepare-release.sh $TAG_NAME && git push origin $BRANCH"
}

main "$@"
