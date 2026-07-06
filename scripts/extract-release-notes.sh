#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <changelog> <tag-or-version> <output-file>" >&2
  exit 2
}

if [ "$#" -ne 3 ]; then
  usage
fi

changelog="$1"
version="${2#v}"
output="$3"

if [ ! -f "$changelog" ]; then
  echo "changelog not found: $changelog" >&2
  exit 1
fi

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
