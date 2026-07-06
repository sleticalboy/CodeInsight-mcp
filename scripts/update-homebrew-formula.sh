#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <tag-or-version> <asset-dir> <formula-path>" >&2
  exit 2
}

if [ "$#" -ne 3 ]; then
  usage
fi

tag="$1"
version="${tag#v}"
asset_dir="$2"
formula_path="$3"

repo="sleticalboy/CodeInsight-mcp"
base_url="https://github.com/${repo}/releases/download/v${version}"

asset_sha() {
  local asset="$1"
  local path="${asset_dir}/${asset}"

  if [ ! -f "$path" ]; then
    echo "release asset not found: $path" >&2
    exit 1
  fi

  shasum -a 256 "$path" | awk '{print $1}'
}

darwin_arm_sha="$(asset_sha codeinsight-aarch64-apple-darwin.tar.gz)"
darwin_intel_sha="$(asset_sha codeinsight-x86_64-apple-darwin.tar.gz)"
linux_arm_sha="$(asset_sha codeinsight-aarch64-unknown-linux-gnu.tar.gz)"
linux_intel_sha="$(asset_sha codeinsight-x86_64-unknown-linux-gnu.tar.gz)"

mkdir -p "$(dirname "$formula_path")"
cat >"$formula_path" <<EOF
class Codeinsight < Formula
  desc "Local-first code intelligence MCP server for AI agents"
  homepage "https://github.com/${repo}"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "${base_url}/codeinsight-aarch64-apple-darwin.tar.gz"
      sha256 "${darwin_arm_sha}"
    end

    on_intel do
      url "${base_url}/codeinsight-x86_64-apple-darwin.tar.gz"
      sha256 "${darwin_intel_sha}"
    end
  end

  on_linux do
    on_arm do
      url "${base_url}/codeinsight-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "${linux_arm_sha}"
    end

    on_intel do
      url "${base_url}/codeinsight-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${linux_intel_sha}"
    end
  end

  def install
    bin.install "codeinsight"
  end

  test do
    assert_match "Usage:", shell_output("#{bin}/codeinsight --help")
  end
end
EOF
