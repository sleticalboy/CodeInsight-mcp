#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$SMOKE_TEMP_DIR"' EXIT

main() {
  local asset_dir="$SMOKE_TEMP_DIR/assets"
  local formula_path="$SMOKE_TEMP_DIR/codeinsight.rb"
  mkdir -p "$asset_dir"

  printf darwin-arm >"$asset_dir/codeinsight-aarch64-apple-darwin.tar.gz"
  printf darwin-intel >"$asset_dir/codeinsight-x86_64-apple-darwin.tar.gz"
  printf linux-arm >"$asset_dir/codeinsight-aarch64-unknown-linux-gnu.tar.gz"
  printf linux-intel >"$asset_dir/codeinsight-x86_64-unknown-linux-gnu.tar.gz"

  "$ROOT_DIR/scripts/update-homebrew-formula.sh" v9.8.7 "$asset_dir" "$formula_path"

  ruby -c "$formula_path"
  grep -q 'releases/download/v9.8.7' "$formula_path"
  grep -q 'codeinsight-aarch64-apple-darwin.tar.gz' "$formula_path"
  grep -q 'codeinsight-x86_64-apple-darwin.tar.gz' "$formula_path"
  grep -q 'codeinsight-aarch64-unknown-linux-gnu.tar.gz' "$formula_path"
  grep -q 'codeinsight-x86_64-unknown-linux-gnu.tar.gz' "$formula_path"

  echo "update Homebrew formula smoke passed"
}

main "$@"
