#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TEMP_DIR=""

cleanup() {
  if [ -n "$SMOKE_TEMP_DIR" ]; then
    rm -rf "$SMOKE_TEMP_DIR"
  fi
}

main() {
  SMOKE_TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  if CODEINSIGHT_VERIFY_RELEASE_NO_MAIN=1 \
    bash -c 'source "$1"; brew_checked "fetching Homebrew formula archive" bash -c "echo SHA256 mismatch >&2; exit 1"' bash "$ROOT_DIR/scripts/verify-release.sh" \
    >"$SMOKE_TEMP_DIR/brew.out" 2>"$SMOKE_TEMP_DIR/brew.err"; then
    echo "brew_checked unexpectedly succeeded with failing brew command" >&2
    exit 1
  fi

  grep -q 'Homebrew command failed while fetching Homebrew formula archive' "$SMOKE_TEMP_DIR/brew.err"
  grep -q 'could not fetch or verify the archive' "$SMOKE_TEMP_DIR/brew.err"

  local tap_dir="$SMOKE_TEMP_DIR/homebrew-tap"
  mkdir -p "$tap_dir"
  git -C "$tap_dir" init -b main >/dev/null
  git -C "$tap_dir" config user.email smoke@example.com
  git -C "$tap_dir" config user.name Smoke
  mkdir -p "$tap_dir/Formula"
  printf 'class Codeinsight < Formula\nend\n' >"$tap_dir/Formula/codeinsight.rb"
  git -C "$tap_dir" add Formula/codeinsight.rb
  git -C "$tap_dir" commit -m init >/dev/null
  printf '# dirty\n' >>"$tap_dir/Formula/codeinsight.rb"

  if CODEINSIGHT_VERIFY_RELEASE_NO_MAIN=1 \
    bash -c 'source "$1"; fast_forward_local_tap "$2"' bash "$ROOT_DIR/scripts/verify-release.sh" "$tap_dir" \
    >"$SMOKE_TEMP_DIR/tap.out" 2>"$SMOKE_TEMP_DIR/tap.err"; then
    echo "fast_forward_local_tap unexpectedly succeeded with dirty tap" >&2
    exit 1
  fi

  grep -q 'local Homebrew tap has uncommitted changes' "$SMOKE_TEMP_DIR/tap.err"
  grep -q 'commit, stash, or discard' "$SMOKE_TEMP_DIR/tap.err"
  grep -q 'CODEINSIGHT_SKIP_HOMEBREW=1' "$SMOKE_TEMP_DIR/tap.err"

  echo "verify-release homebrew failure smoke passed"
}

main "$@"
