#!/usr/bin/env bash
set -euo pipefail

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/extract-release-notes.sh [--summary] [--max-items N] <changelog> <tag-or-version|latest> <output-file>

Options:
  --summary      Write compact GitHub Release notes with a bounded highlight list.
  --max-items N  Maximum bullet items in summary mode. Default: 12.
EOF
  exit "$status"
}

summary=0
max_items="${CODEINSIGHT_RELEASE_NOTES_MAX_ITEMS:-12}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --summary)
      summary=1
      shift
      ;;
    --max-items)
      if [ "$#" -lt 2 ]; then
        usage
      fi
      max_items="$2"
      shift 2
      ;;
    -h | --help)
      usage 0
      ;;
    --)
      shift
      break
      ;;
    -*)
      usage
      ;;
    *)
      break
      ;;
  esac
done

case "$max_items" in
  '' | *[!0-9]*)
    echo "--max-items must be a positive integer" >&2
    exit 2
    ;;
esac
if [ "$max_items" -lt 1 ]; then
  echo "--max-items must be a positive integer" >&2
  exit 2
fi

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

temp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$temp_dir"
}
trap cleanup EXIT INT TERM

full_output="$temp_dir/release-notes-full.md"

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
' "$changelog" | sed '/./,$!d' >"$full_output"

if [ ! -s "$full_output" ]; then
  echo "release notes for v$version are empty" >&2
  exit 1
fi

if [ "$summary" -eq 0 ]; then
  cp "$full_output" "$output"
  exit 0
fi

total_items="$(awk '/^- / { count++ } END { print count + 0 }' "$full_output")"
if [ "$total_items" -le "$max_items" ]; then
  cp "$full_output" "$output"
  exit 0
fi

{
  printf '### Highlights\n\n'
  awk -v max_items="$max_items" '
    /^- / {
      count++
      if (count <= max_items) {
        print
      }
    }
  ' "$full_output"
  printf '\n'
  printf '_This release has %s changelog entries. See `CHANGELOG.md` for the full list._\n' "$total_items"
} >"$output"
