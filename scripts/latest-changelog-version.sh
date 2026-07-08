#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <changelog>" >&2
  exit 2
}

if [ "$#" -ne 1 ]; then
  usage
fi

changelog="$1"

if [ ! -f "$changelog" ]; then
  echo "changelog not found: $changelog" >&2
  exit 1
fi

awk '
  /^## \[[0-9]+\.[0-9]+\.[0-9]+\]([[:space:]]|-|$)/ {
    version = $0
    sub(/^## \[/, "", version)
    sub(/\].*/, "", version)
    print "v" version
    found = 1
    exit
  }
  END {
    if (!found) {
      exit 3
    }
  }
' "$changelog"
