#!/usr/bin/env bash
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$SCRIPT_ROOT}"
RELEASE_WORKFLOW_GUARD_SCRIPT="${CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT:-$ROOT_DIR/scripts/release-workflow-guard-smoke.sh}"
RELEASE_PRETAG_CHECK_SCRIPT="${CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT:-$ROOT_DIR/scripts/release-pretag-check.sh}"
REPO_ARG=()
REPO=""
BRANCH="main"
HEAD_SHA=""
TAG_NAME=""
METADATA_SUMMARY=""

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
  CODEINSIGHT_ROOT_DIR=/path/to/repo
EOF
  exit "$status"
}

fail() {
  echo "release tag preflight failed: $*" >&2
  exit 1
}

check_release_metadata() {
  local tag="$1"
  local version="${tag#v}"

  ruby - "$ROOT_DIR" "$tag" "$version" <<'RUBY'
root_dir = ARGV.fetch(0)
tag = ARGV.fetch(1)
version = ARGV.fetch(2)

def fail!(message)
  warn("release tag preflight failed: #{message}")
  exit(1)
end

def read_file(path)
  File.read(path)
rescue Errno::ENOENT
  fail!("missing required release metadata file: #{path}")
end

cargo_path = File.join(root_dir, "Cargo.toml")
cargo = read_file(cargo_path)
package = cargo.match(/^\[package\]\n(?<body>.*?)(?=^\[|\z)/m)
fail!("Cargo.toml [package] section not found") unless package
cargo_version = package[:body][/^version = "([^"]+)"/, 1]
fail!("Cargo.toml package version not found") unless cargo_version
fail!("Cargo.toml version #{cargo_version} does not match #{version}") unless cargo_version == version

install_path = File.join(root_dir, "docs", "install.md")
install_doc = read_file(install_path)
install_version = install_doc[/CODEINSIGHT_VERSION=(v\d+\.\d+\.\d+)/, 1]
unless install_version == tag
  fail!("docs/install.md CODEINSIGHT_VERSION does not match #{tag}")
end

changelog_path = File.join(root_dir, "CHANGELOG.md")
changelog = read_file(changelog_path)
changelog_match = changelog.match(/^## \[#{Regexp.escape(version)}\] - (?<date>\d{4}-\d{2}-\d{2})$/)
unless changelog_match
  fail!("CHANGELOG.md release section not found for #{version}")
end

puts "metadata_cargo: #{cargo_version}"
puts "metadata_install: #{install_version}"
puts "metadata_changelog: #{version} (#{changelog_match[:date]})"
RUBY
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
  METADATA_SUMMARY="$(check_release_metadata "$TAG_NAME")"
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
  "$RELEASE_PRETAG_CHECK_SCRIPT" "${REPO_ARG[@]}" --head-sha "$HEAD_SHA" "$BRANCH"

  echo "release tag preflight passed"
  echo "next: git tag -a $TAG_NAME -m \"$TAG_NAME\" && git push origin $TAG_NAME"
}

main "$@"
