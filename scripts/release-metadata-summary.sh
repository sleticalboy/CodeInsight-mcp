#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
CONTEXT="${CODEINSIGHT_RELEASE_METADATA_CONTEXT:-release metadata summary}"
TAG_NAME=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-metadata-summary.sh <tag>

Validates that release metadata is prepared for the target tag and prints the
stable metadata summary lines used by release preflight and evidence scripts.

Environment:
  CODEINSIGHT_ROOT_DIR=/path/to/repo
  CODEINSIGHT_RELEASE_METADATA_CONTEXT="release tag preflight"
EOF
  exit "$status"
}

fail() {
  echo "$CONTEXT failed: $*" >&2
  exit 1
}

normalize_tag() {
  local tag="$1"
  case "$tag" in
    v*) printf "%s" "$tag" ;;
    *) printf "v%s" "$tag" ;;
  esac
}

if [ "$#" -ne 1 ]; then
  usage
fi

TAG_NAME="$(normalize_tag "$1")"
if [[ ! "$TAG_NAME" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  fail "tag must look like vX.Y.Z or X.Y.Z: $TAG_NAME"
fi

version="${TAG_NAME#v}"

ruby - "$ROOT_DIR" "$TAG_NAME" "$version" "$CONTEXT" <<'RUBY'
root_dir = ARGV.fetch(0)
tag = ARGV.fetch(1)
version = ARGV.fetch(2)
context = ARGV.fetch(3)

def fail!(context, message)
  warn("#{context} failed: #{message}")
  exit(1)
end

def read_file(context, path)
  File.read(path)
rescue Errno::ENOENT
  fail!(context, "missing required release metadata file: #{path}")
end

cargo_path = File.join(root_dir, "Cargo.toml")
cargo = read_file(context, cargo_path)
package = cargo.match(/^\[package\]\n(?<body>.*?)(?=^\[|\z)/m)
fail!(context, "Cargo.toml [package] section not found") unless package
cargo_version = package[:body][/^version = "([^"]+)"/, 1]
fail!(context, "Cargo.toml package version not found") unless cargo_version
unless cargo_version == version
  fail!(context, "Cargo.toml version #{cargo_version} does not match #{version}")
end

install_path = File.join(root_dir, "docs", "install.md")
install_doc = read_file(context, install_path)
install_version = install_doc[/CODEINSIGHT_VERSION=(v\d+\.\d+\.\d+)/, 1]
unless install_version == tag
  fail!(context, "docs/install.md CODEINSIGHT_VERSION does not match #{tag}")
end

changelog_path = File.join(root_dir, "CHANGELOG.md")
changelog = read_file(context, changelog_path)
changelog_match = changelog.match(/^## \[#{Regexp.escape(version)}\] - (?<date>\d{4}-\d{2}-\d{2})$/)
unless changelog_match
  fail!(context, "CHANGELOG.md release section not found for #{version}")
end

puts "metadata_cargo: #{cargo_version}"
puts "metadata_install: #{install_version}"
puts "metadata_changelog: #{version} (#{changelog_match[:date]})"
RUBY
