#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_WORKFLOW_GUARD_SCRIPT="${CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT:-$ROOT_DIR/scripts/release-workflow-guard-smoke.sh}"
RELEASE_PRETAG_CHECK_SCRIPT="${CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT:-$ROOT_DIR/scripts/release-pretag-check.sh}"
REPO_ARG=()
BRANCH="main"
HEAD_SHA=""
TAG_NAME=""

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
target commit already has successful CI plus a valid benchmark subset artifact.

Options:
  --repo OWNER/REPO  Pass an explicit GitHub repository to pretag validation.
  --head-sha SHA     Check this commit instead of the current HEAD.
  -h, --help         Show this help.

Environment:
  CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT=scripts/release-workflow-guard-smoke.sh
  CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT=scripts/release-pretag-check.sh
EOF
  exit "$status"
}

fail() {
  echo "release tag preflight failed: $*" >&2
  exit 1
}

normalize_tag() {
  local tag="$1"
  case "$tag" in
    v*) printf "%s" "$tag" ;;
    *) printf "v%s" "$tag" ;;
  esac
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
  if [[ ! "$TAG_NAME" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
    fail "tag must look like vX.Y.Z or X.Y.Z: $TAG_NAME"
  fi

  if [ -z "$HEAD_SHA" ]; then
    HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"
  fi

  if git -C "$ROOT_DIR" rev-parse -q --verify "refs/tags/$TAG_NAME" >/dev/null; then
    fail "local tag already exists: $TAG_NAME"
  fi
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

  "$RELEASE_WORKFLOW_GUARD_SCRIPT"
  "$RELEASE_PRETAG_CHECK_SCRIPT" "${REPO_ARG[@]}" --head-sha "$HEAD_SHA" "$BRANCH"

  echo "release tag preflight passed"
  echo "next: git tag -a $TAG_NAME -m \"$TAG_NAME\" && git push origin $TAG_NAME"
}

main "$@"
