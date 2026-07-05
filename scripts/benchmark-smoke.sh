#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${CODEINSIGHT_BENCH_WORKDIR:-${TMPDIR:-/tmp}/codeinsight-benchmark}"
OUTPUT="${CODEINSIGHT_BENCH_OUTPUT:-$ROOT_DIR/docs/benchmark-v0.1.md}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-$ROOT_DIR/target/release/codeinsight}"

REPO_NAMES=(
  "p-limit"
  "itsdangerous"
  "go-example"
  "memchr"
)

REPO_URLS=(
  "https://github.com/sindresorhus/p-limit.git"
  "https://github.com/pallets/itsdangerous.git"
  "https://github.com/golang/example.git"
  "https://github.com/BurntSushi/memchr.git"
)

REPO_LANGUAGES=(
  "TypeScript"
  "Python"
  "Go"
  "Rust"
)

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

clone_repo() {
  local name="$1"
  local url="$2"
  local repo_dir="$WORK_DIR/repos/$name"

  rm -rf "$repo_dir"
  git clone --quiet --depth 1 "$url" "$repo_dir"
  rm -rf "$repo_dir/.codeinsight"
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

write_report_header() {
  local generated_at
  local display_bin
  generated_at="$(date -u +"%Y-%m-%d %H:%M:%S UTC")"
  display_bin="$CODEINSIGHT_BIN"
  display_bin="${display_bin/#$ROOT_DIR\//}"

  cat >"$OUTPUT" <<EOF
# CodeInsight v0.1 Smoke Benchmark

Generated at: $generated_at

This is a smoke benchmark, not a controlled performance benchmark. It verifies
that CodeInsight can index real public repositories across the MVP language set
and produce stable project summaries without crashing.

Environment:

- Command: \`$display_bin\`
- Work directory: temporary clone directory
- Index mode: forced clean index per repository

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | DB size |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
EOF
}

append_summary_row() {
  local name="$1"
  local language="$2"
  local repo_dir="$3"
  local index_json="$4"
  local overview_json="$5"

  local commit files lines symbols skipped errors duration db_size
  commit="$(git -C "$repo_dir" rev-parse --short HEAD)"
  files="$(json_value "$index_json" '.indexed_files')"
  lines="$(json_value "$overview_json" '[.languages[].lines] | add // 0')"
  symbols="$(json_value "$index_json" '.symbols')"
  skipped="$(json_value "$index_json" '.skipped_files')"
  errors="$(json_value "$index_json" '.errors | length')"
  duration="$(json_value "$index_json" '.duration_ms')"
  db_size="$(du -h "$repo_dir/.codeinsight/index.db" | awk '{print $1}')"

  printf "| %s | %s | \`%s\` | %s | %s | %s | %s | %s | %s | %s |\n" \
    "$name" "$language" "$commit" "$files" "$lines" "$symbols" "$skipped" "$errors" "$duration" "$db_size" \
    >>"$OUTPUT"
}

append_detail_section() {
  local name="$1"
  local repo_url="$2"
  local repo_dir="$3"
  local index_json="$4"
  local overview_json="$5"

  {
    echo
    echo "## $name"
    echo
    echo "- URL: $repo_url"
    echo "- Commit: \`$(git -C "$repo_dir" rev-parse HEAD)\`"
    echo "- Indexed files: $(json_value "$index_json" '.indexed_files')"
    echo "- Symbols: $(json_value "$index_json" '.symbols')"
    echo "- Duration: $(json_value "$index_json" '.duration_ms') ms"
    echo
    echo "Language breakdown:"
    echo
    echo "| Language | Files | Lines |"
    echo "| --- | ---: | ---: |"
  } >>"$OUTPUT"

  jq -r '.languages[] | "| \(.language) | \(.files) | \(.lines) |"' "$overview_json" >>"$OUTPUT"

  local error_count
  error_count="$(json_value "$index_json" '.errors | length')"
  if [ "$error_count" -gt 0 ]; then
    {
      echo
      echo "Index errors:"
      echo
    } >>"$OUTPUT"
    jq -r '.errors[:10][] | "- `\(.file)` during `\(.stage)`: \(.message)"' "$index_json" >>"$OUTPUT"
  fi
}

main() {
  require_command git
  require_command jq
  require_command cargo
  require_command du
  require_command awk

  mkdir -p "$WORK_DIR/results" "$(dirname "$OUTPUT")"

  echo "building release binary"
  cargo build --locked --release --manifest-path "$ROOT_DIR/Cargo.toml"

  write_report_header

  for i in "${!REPO_NAMES[@]}"; do
    name="${REPO_NAMES[$i]}"
    url="${REPO_URLS[$i]}"
    language="${REPO_LANGUAGES[$i]}"
    repo_dir="$WORK_DIR/repos/$name"
    index_json="$WORK_DIR/results/$name-index.json"
    overview_json="$WORK_DIR/results/$name-overview.json"

    echo "benchmarking $name"
    clone_repo "$name" "$url"
    "$CODEINSIGHT_BIN" index "$repo_dir" --force >"$index_json"
    "$CODEINSIGHT_BIN" overview "$repo_dir" >"$overview_json"
    append_summary_row "$name" "$language" "$repo_dir" "$index_json" "$overview_json"
  done

  cat >>"$OUTPUT" <<EOF

## Details
EOF

  for i in "${!REPO_NAMES[@]}"; do
    name="${REPO_NAMES[$i]}"
    url="${REPO_URLS[$i]}"
    repo_dir="$WORK_DIR/repos/$name"
    index_json="$WORK_DIR/results/$name-index.json"
    overview_json="$WORK_DIR/results/$name-overview.json"
    append_detail_section "$name" "$url" "$repo_dir" "$index_json" "$overview_json"
  done

  echo "wrote $OUTPUT"
}

main "$@"
