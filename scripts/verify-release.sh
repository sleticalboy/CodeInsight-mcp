#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="${CODEINSIGHT_REPO:-sleticalboy/CodeInsight-mcp}"
IMAGE="${CODEINSIGHT_DOCKER_IMAGE:-ghcr.io/sleticalboy/codeinsight-mcp}"
HOMEBREW_TAP="${CODEINSIGHT_HOMEBREW_TAP:-sleticalboy/tap}"
HOMEBREW_REPO="${CODEINSIGHT_HOMEBREW_REPO:-sleticalboy/homebrew-tap}"
TEMP_DIR=""

EXPECTED_ASSETS=(
  codeinsight-aarch64-apple-darwin.tar.gz
  codeinsight-aarch64-unknown-linux-gnu.tar.gz
  codeinsight-x86_64-apple-darwin.tar.gz
  codeinsight-x86_64-unknown-linux-gnu.tar.gz
)

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/verify-release.sh <tag-or-version>

Verifies a published CodeInsight release:
- GitHub Release metadata, notes, and platform assets
- public install script for the current host platform
- GHCR multi-arch Docker manifests and container version output
- Homebrew tap formula and fetch checksum

If the Homebrew tap update is still waiting in an open PR, this script reports
the pending PR and exits non-zero. Merge the tap PR, then rerun verification.

Environment:
  CODEINSIGHT_REPO=sleticalboy/CodeInsight-mcp
  CODEINSIGHT_DOCKER_IMAGE=ghcr.io/sleticalboy/codeinsight-mcp
  CODEINSIGHT_HOMEBREW_TAP=sleticalboy/tap
  CODEINSIGHT_HOMEBREW_REPO=sleticalboy/homebrew-tap
  CODEINSIGHT_SKIP_DOCKER=1
  CODEINSIGHT_SKIP_HOMEBREW=1
EOF
  exit "$status"
}

log() {
  printf '\n==> %s\n' "$*"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

download_installer() {
  local installer_url="$1"
  local output="$2"
  local attempt

  for attempt in 1 2 3; do
    if curl --fail --silent --show-error --location \
      --connect-timeout 20 --max-time 60 \
      "$installer_url" \
      -o "$output"; then
      return
    fi
    echo "installer download failed, retrying (${attempt}/3): $installer_url" >&2
    sleep 2
  done

  echo "falling back to GitHub API for scripts/install.sh" >&2
  gh api "repos/${REPO}/contents/scripts/install.sh?ref=main" --jq .content | base64 --decode >"$output"
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

normalize_tag() {
  local input="$1"
  case "$input" in
    v*) printf '%s' "$input" ;;
    *) printf 'v%s' "$input" ;;
  esac
}

version_from_tag() {
  printf '%s' "${1#v}"
}

verify_github_release() {
  local tag="$1"
  local release_json asset_count

  log "Verify GitHub Release ${tag}"
  release_json="$(gh release view "$tag" --repo "$REPO" --json tagName,isDraft,isPrerelease,url,assets,body)"

  jq -e \
    --arg tag "$tag" \
    '.tagName == $tag and .isDraft == false and .isPrerelease == false and (.body | length > 0)' \
    >/dev/null <<<"$release_json"

  for asset in "${EXPECTED_ASSETS[@]}"; do
    jq -e --arg asset "$asset" '.assets[] | select(.name == $asset and .size > 0)' >/dev/null <<<"$release_json"
  done

  asset_count="$(jq '.assets | length' <<<"$release_json")"
  printf 'release: %s\n' "$(jq -r .url <<<"$release_json")"
  printf 'assets: %s\n' "$asset_count"
}

verify_release_notes() {
  local tag="$1"
  local notes_file

  log "Verify CHANGELOG extraction for ${tag}"
  notes_file="$TEMP_DIR/release-notes.md"
  "$ROOT_DIR/scripts/extract-release-notes.sh" "$ROOT_DIR/CHANGELOG.md" "$tag" "$notes_file"
  test -s "$notes_file"
  if grep -Eq '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' "$notes_file"; then
    echo "release notes include another release heading" >&2
    exit 1
  fi
  sed -n '1,12p' "$notes_file"
}

verify_install_script() {
  local tag="$1"
  local version="$2"
  local install_dir version_json
  local installer_url installer_path

  log "Verify public install script for ${tag}"
  install_dir="$TEMP_DIR/install/bin"
  installer_path="$TEMP_DIR/install.sh"
  installer_url="${CODEINSIGHT_INSTALL_SCRIPT_URL:-https://raw.githubusercontent.com/${REPO}/main/scripts/install.sh}"
  mkdir -p "$install_dir"

  download_installer "$installer_url" "$installer_path"

  INSTALL_DIR="$install_dir" CODEINSIGHT_VERSION="$tag" sh "$installer_path"

  test -x "$install_dir/codeinsight"
  version_json="$("$install_dir/codeinsight" version)"
  jq -e --arg version "$version" '.name == "codeinsight" and .version == $version' >/dev/null <<<"$version_json"
  printf '%s\n' "$version_json"
}

docker_manifest_platforms() {
  local image_ref="$1"
  docker buildx imagetools inspect "$image_ref" | awk '/Platform:/ {print $2}' | sort -u
}

docker_manifest_digest() {
  local image_ref="$1"
  docker buildx imagetools inspect "$image_ref" | awk '/^Digest:/ {print $2; exit}'
}

verify_docker() {
  local version="$1"
  local tag_ref="${IMAGE}:${version}"
  local latest_ref="${IMAGE}:latest"
  local tag_digest latest_digest version_json

  if [ "${CODEINSIGHT_SKIP_DOCKER:-}" = "1" ]; then
    log "Skip Docker verification"
    return
  fi

  require_command docker

  log "Verify Docker manifests"
  tag_digest="$(docker_manifest_digest "$tag_ref")"
  latest_digest="$(docker_manifest_digest "$latest_ref")"
  test -n "$tag_digest"
  test "$tag_digest" = "$latest_digest"

  docker_manifest_platforms "$tag_ref" | grep -qx 'linux/amd64'
  docker_manifest_platforms "$tag_ref" | grep -qx 'linux/arm64'
  printf 'docker digest: %s\n' "$tag_digest"

  log "Verify Docker version output"
  version_json="$(docker run --rm --platform linux/arm64 "$tag_ref" version)"
  jq -e --arg version "$version" '.name == "codeinsight" and .version == $version and .target_arch == "aarch64"' >/dev/null <<<"$version_json"
  printf '%s\n' "$version_json"

  version_json="$(docker run --rm --platform linux/amd64 "$tag_ref" version)"
  jq -e --arg version "$version" '.name == "codeinsight" and .version == $version and .target_arch == "x86_64"' >/dev/null <<<"$version_json"
  printf '%s\n' "$version_json"
}

verify_homebrew_remote_formula() {
  local tag="$1"
  local formula

  log "Verify remote Homebrew formula"
  formula="$(gh api "repos/${HOMEBREW_REPO}/contents/Formula/codeinsight.rb" --jq .content | base64 --decode)"

  if ! homebrew_formula_has_tag "$formula" "$tag"; then
    if report_pending_homebrew_pr "$tag"; then
      exit 1
    fi
    echo "remote Homebrew formula does not reference ${tag}: ${HOMEBREW_REPO}/Formula/codeinsight.rb" >&2
    exit 1
  fi

  printf '%s\n' "$formula" >"$TEMP_DIR/codeinsight.rb"
  ruby -c "$TEMP_DIR/codeinsight.rb" >/dev/null
  printf 'remote formula: %s\n' "$tag"
}

homebrew_formula_has_tag() {
  local formula="$1"
  local tag="$2"

  grep -q "releases/download/${tag}/codeinsight-aarch64-apple-darwin.tar.gz" <<<"$formula" &&
    grep -q "releases/download/${tag}/codeinsight-x86_64-apple-darwin.tar.gz" <<<"$formula" &&
    grep -q "releases/download/${tag}/codeinsight-aarch64-unknown-linux-gnu.tar.gz" <<<"$formula" &&
    grep -q "releases/download/${tag}/codeinsight-x86_64-unknown-linux-gnu.tar.gz" <<<"$formula"
}

report_pending_homebrew_pr() {
  local tag="$1"
  local branch="codeinsight-${tag}"
  local pr_json number url title

  pr_json="$(gh pr list \
    --repo "$HOMEBREW_REPO" \
    --head "$branch" \
    --base main \
    --state open \
    --json number,title,url \
    --jq '.[0] // empty')" || true

  if [ -z "$pr_json" ]; then
    return 1
  fi

  number="$(jq -r .number <<<"$pr_json")"
  title="$(jq -r .title <<<"$pr_json")"
  url="$(jq -r .url <<<"$pr_json")"
  echo "remote Homebrew formula does not reference ${tag} yet." >&2
  echo "pending Homebrew tap PR #${number}: ${title}" >&2
  echo "$url" >&2
  echo "merge the tap PR, then rerun: scripts/verify-release.sh ${tag}" >&2
}

tap_path() {
  HOMEBREW_NO_AUTO_UPDATE=1 brew --repository "$HOMEBREW_TAP" 2>/dev/null || true
}

fast_forward_local_tap() {
  local tap_dir="$1"

  if [ -z "$tap_dir" ] || [ ! -d "$tap_dir/.git" ]; then
    return
  fi

  if [ -n "$(git -C "$tap_dir" status --porcelain)" ]; then
    echo "local Homebrew tap has uncommitted changes: $tap_dir" >&2
    exit 1
  fi

  if ! git -C "$tap_dir" fetch origin main; then
    echo "warning: could not refresh local Homebrew tap; using current local tap state" >&2
    return
  fi
  git -C "$tap_dir" merge --ff-only origin/main
}

verify_homebrew_fetch() {
  local version="$1"
  local tap_dir stable_version

  if [ "${CODEINSIGHT_SKIP_HOMEBREW:-}" = "1" ]; then
    log "Skip Homebrew verification"
    return
  fi

  require_command brew

  log "Verify Homebrew fetch"
  HOMEBREW_NO_AUTO_UPDATE=1 brew tap "$HOMEBREW_TAP" >/dev/null
  tap_dir="$(tap_path)"
  fast_forward_local_tap "$tap_dir"

  stable_version="$(HOMEBREW_NO_AUTO_UPDATE=1 brew info --json=v2 "${HOMEBREW_TAP}/codeinsight" | jq -r '.formulae[0].versions.stable')"
  test "$stable_version" = "$version"
  HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --formula "${HOMEBREW_TAP}/codeinsight" --force
}

main() {
  if [ "$#" -eq 1 ] && { [ "$1" = "-h" ] || [ "$1" = "--help" ]; }; then
    usage 0
  fi

  if [ "$#" -ne 1 ]; then
    usage
  fi

  require_command gh
  require_command jq
  require_command curl
  require_command ruby

  local tag version
  tag="$(normalize_tag "$1")"
  version="$(version_from_tag "$tag")"

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  verify_github_release "$tag"
  verify_release_notes "$tag"
  verify_install_script "$tag" "$version"
  verify_docker "$version"
  verify_homebrew_remote_formula "$tag"
  verify_homebrew_fetch "$version"

  log "Release verification passed"
  printf 'tag: %s\n' "$tag"
}

main "$@"
