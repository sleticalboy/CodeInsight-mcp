#!/usr/bin/env bash
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$SCRIPT_ROOT}"
DRY_RUN=0

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/prepare-release.sh [--dry-run] <tag-or-version>

Prepares a CodeInsight release by updating:
- Cargo.toml package version
- Cargo.lock package version, unless CODEINSIGHT_SKIP_CARGO_CHECK=1
- README install example
- CHANGELOG Unreleased section into a dated release section

Environment:
  CODEINSIGHT_ROOT_DIR=/path/to/repo
  CODEINSIGHT_RELEASE_DATE=YYYY-MM-DD
  CODEINSIGHT_SKIP_CARGO_CHECK=1
  CODEINSIGHT_ALLOW_EMPTY_CHANGELOG=1
EOF
  exit "$status"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h | --help)
      usage 0
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "unknown option: $1" >&2
      usage
      ;;
    *)
      break
      ;;
  esac
done

if [ "$#" -ne 1 ]; then
  usage
fi

tag="$1"
version="${tag#v}"
tag="v${version}"
release_date="${CODEINSIGHT_RELEASE_DATE:-$(date +%F)}"

case "$version" in
  *[!0-9.]* | *.*.*.* | .* | *.)
    echo "release version must look like vX.Y.Z or X.Y.Z: $1" >&2
    exit 1
    ;;
esac

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "release version must look like vX.Y.Z or X.Y.Z: $1" >&2
  exit 1
fi

work_dir="$ROOT_DIR"
temp_dir=""

cleanup() {
  if [ -n "$temp_dir" ]; then
    rm -rf "$temp_dir"
  fi
}
trap cleanup EXIT INT TERM

if [ "$DRY_RUN" -eq 1 ]; then
  temp_dir="$(mktemp -d)"
  mkdir -p "$temp_dir"
  cp "$ROOT_DIR/Cargo.toml" "$ROOT_DIR/Cargo.lock" "$ROOT_DIR/README.md" "$ROOT_DIR/CHANGELOG.md" "$temp_dir/"
  work_dir="$temp_dir"
fi

export CODEINSIGHT_PREPARE_VERSION="$version"
export CODEINSIGHT_PREPARE_TAG="$tag"
export CODEINSIGHT_PREPARE_DATE="$release_date"
export CODEINSIGHT_PREPARE_WORK_DIR="$work_dir"

ruby <<'RUBY'
version = ENV.fetch("CODEINSIGHT_PREPARE_VERSION")
tag = ENV.fetch("CODEINSIGHT_PREPARE_TAG")
date = ENV.fetch("CODEINSIGHT_PREPARE_DATE")
work_dir = ENV.fetch("CODEINSIGHT_PREPARE_WORK_DIR")

def path(work_dir, name)
  File.join(work_dir, name)
end

def parse_version(value)
  parts = value.split(".").map do |part|
    Integer(part, exception: false) || abort("invalid semantic version: #{value}")
  end
  abort("invalid semantic version: #{value}") unless parts.length == 3
  parts
end

target_version = parse_version(version)

cargo_path = path(work_dir, "Cargo.toml")
cargo = File.read(cargo_path)
package_seen = false
changed = false
next_cargo = cargo.lines.map do |line|
  if line =~ /^\[package\]/
    package_seen = true
    line
  elsif package_seen && line =~ /^\[/
    package_seen = false
    line
  elsif package_seen && line =~ /^version = "([^"]+)"/
    current = Regexp.last_match(1)
    if current == version
      abort("Cargo.toml is already at version #{version}")
    end
    current_version = parse_version(current)
    if (target_version <=> current_version) <= 0
      abort("release version #{version} must be greater than current Cargo.toml version #{current}")
    end
    changed = true
    %(version = "#{version}"\n)
  else
    line
  end
end.join
abort("Cargo.toml package version not found") unless changed

readme_path = path(work_dir, "README.md")
readme = File.read(readme_path)
readme_changed = readme.gsub!(/CODEINSIGHT_VERSION=v\d+\.\d+\.\d+/, "CODEINSIGHT_VERSION=#{tag}")
abort("README install version example not found") unless readme_changed

changelog_path = path(work_dir, "CHANGELOG.md")
changelog = File.read(changelog_path)
abort("CHANGELOG already contains #{version}") if changelog.match?(/^## \[#{Regexp.escape(version)}\]/)

unreleased_match = changelog.match(/\A(?<prefix>.*?^## \[Unreleased\]\n)(?<body>.*?)(?=^## \[\d+\.\d+\.\d+\])/m)
abort("CHANGELOG Unreleased section not found") unless unreleased_match

prefix = unreleased_match[:prefix]
body = unreleased_match[:body].strip
if body.empty? && ENV["CODEINSIGHT_ALLOW_EMPTY_CHANGELOG"] != "1"
  abort("CHANGELOG Unreleased section is empty; add release notes or set CODEINSIGHT_ALLOW_EMPTY_CHANGELOG=1")
end

suffix = changelog[unreleased_match.end(0)..]
new_section = "## [#{version}] - #{date}\n\n#{body}\n\n"
next_changelog = "#{prefix}\n#{new_section}#{suffix}"

File.write(cargo_path, next_cargo)
File.write(readme_path, readme)
File.write(changelog_path, next_changelog)
RUBY

if [ "$DRY_RUN" -eq 1 ]; then
  for file in Cargo.toml README.md CHANGELOG.md; do
    diff -u "$ROOT_DIR/$file" "$work_dir/$file" || true
  done
  echo "dry run completed for $tag"
  exit 0
fi

if [ "${CODEINSIGHT_SKIP_CARGO_CHECK:-}" != "1" ]; then
  cargo check --offline
fi

echo "release prepared: $tag"
echo "date: $release_date"
