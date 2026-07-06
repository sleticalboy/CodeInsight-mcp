#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <tag-or-version> <asset-dir>" >&2
  exit 2
}

if [ "$#" -ne 2 ]; then
  usage
fi

tag="$1"
asset_dir="$2"

if [ ! -d "$asset_dir" ]; then
  echo "release asset directory not found: $asset_dir" >&2
  exit 1
fi

if [ -z "${HOMEBREW_TAP_TOKEN:-}" ]; then
  echo "HOMEBREW_TAP_TOKEN is not configured; skipping Homebrew tap update."
  exit 0
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

tap_dir="$tmp_dir/homebrew-tap"
git clone "https://x-access-token:${HOMEBREW_TAP_TOKEN}@github.com/sleticalboy/homebrew-tap.git" "$tap_dir"

scripts/update-homebrew-formula.sh "$tag" "$asset_dir" "$tap_dir/Formula/codeinsight.rb"
ruby -c "$tap_dir/Formula/codeinsight.rb"

cd "$tap_dir"
if [ -z "$(git status --porcelain -- Formula/codeinsight.rb)" ]; then
  echo "Homebrew tap formula is already up to date."
  exit 0
fi

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

branch="codeinsight-${tag}"
git checkout -B "$branch"
git add Formula/codeinsight.rb
git commit -m "Update codeinsight to ${tag}"
git push --force-with-lease origin "$branch"

export GH_TOKEN="${GH_TOKEN:-$HOMEBREW_TAP_TOKEN}"
if gh pr view "$branch" --repo sleticalboy/homebrew-tap >/dev/null 2>&1; then
  gh pr edit "$branch" \
    --repo sleticalboy/homebrew-tap \
    --title "Update codeinsight to ${tag}" \
    --body "Updates the CodeInsight Homebrew formula for ${tag}."
else
  gh pr create \
    --repo sleticalboy/homebrew-tap \
    --head "$branch" \
    --base main \
    --title "Update codeinsight to ${tag}" \
    --body "Updates the CodeInsight Homebrew formula for ${tag}."
fi
