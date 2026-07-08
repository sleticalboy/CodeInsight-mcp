#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <changelog> <tag-or-version|latest> <output-file>" >&2
  exit 2
}

if [ "$#" -ne 3 ]; then
  usage
fi

changelog="$1"
requested_version="$2"
output="$3"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ ! -f "$changelog" ]; then
  echo "changelog not found: $changelog" >&2
  exit 1
fi

if [ "$requested_version" = "latest" ]; then
  version="$("$script_dir/latest-changelog-version.sh" "$changelog")"
else
  version="$requested_version"
fi

version="${version#v}"

awk -v version="$version" '
  BEGIN {
    in_section = 0
    found = 0
  }
  $0 ~ "^## \\[" version "\\]([[:space:]]|-|$)" {
    in_section = 1
    found = 1
    next
  }
  in_section && $0 ~ "^## " {
    exit
  }
  in_section {
    print
  }
  END {
    if (!found) {
      exit 3
    }
  }
' "$changelog" | sed '/./,$!d' >"$output"

if [ ! -s "$output" ]; then
  echo "release notes for v$version are empty" >&2
  exit 1
fi
