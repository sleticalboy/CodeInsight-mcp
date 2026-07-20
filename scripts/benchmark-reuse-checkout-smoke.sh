#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codeinsight-benchmark-reuse.XXXXXX")"

cleanup() {
  rm -rf "$TEMP_DIR"
}

fail() {
  echo "benchmark reuse checkout smoke failed: $*" >&2
  exit 1
}

write_fake_tools() {
  mkdir -p "$TEMP_DIR/bin"

  cat >"$TEMP_DIR/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  build)
    exit 0
    ;;
  metadata)
    printf '{"target_directory":"%s"}\n' "${CODEINSIGHT_FAKE_TARGET_DIR:?}"
    exit 0
    ;;
esac

echo "unexpected fake cargo invocation: $*" >&2
exit 1
EOF

  cat >"$TEMP_DIR/bin/git" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

LOG_FILE="${CODEINSIGHT_FAKE_GIT_LOG:?}"

if [ "${1:-}" = "-C" ]; then
  repo_dir="$2"
  shift 2
  case "$1" in
    rev-parse)
      case "${2:-}" in
        --is-inside-work-tree)
          [ -f "$repo_dir/.valid-git" ] || exit 128
          echo "true"
          ;;
        --short)
          echo "abcdef0"
          ;;
        HEAD)
          echo "abcdef0123456789"
          ;;
        *)
          echo "unexpected fake git rev-parse invocation: $*" >&2
          exit 1
          ;;
      esac
      exit 0
      ;;
    reset)
      [ -f "$repo_dir/.valid-git" ] || exit 128
      touch "$repo_dir/.reset-called"
      printf 'reset\t%s\n' "$repo_dir" >>"$LOG_FILE"
      printf 'export const limit = 1;\n' >"$repo_dir/index.js"
      exit 0
      ;;
    clean)
      [ -f "$repo_dir/.valid-git" ] || exit 128
      touch "$repo_dir/.clean-called"
      rm -f "$repo_dir/dirty.tmp"
      printf 'clean\t%s\n' "$repo_dir" >>"$LOG_FILE"
      exit 0
      ;;
  esac
fi

if [ "${1:-}" = "-c" ]; then
  while [ "${1:-}" = "-c" ]; do
    shift 2
  done
fi

if [ "${1:-}" = "clone" ]; then
  repo_dir="${@: -1}"
  mkdir -p "$repo_dir/.git"
  touch "$repo_dir/.valid-git"
  printf 'export const limit = 1;\n' >"$repo_dir/index.js"
  printf 'clone\t%s\n' "$repo_dir" >>"$LOG_FILE"
  exit 0
fi

echo "unexpected fake git invocation: $*" >&2
exit 1
EOF

  cat >"$TEMP_DIR/bin/codeinsight" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  index)
    repo_dir="$2"
    mkdir -p "$repo_dir/.codeinsight"
    printf 'db\n' >"$repo_dir/.codeinsight/index.db"
    printf '{"indexed_files":1,"symbols":1,"skipped_files":0,"errors":[],"duration_ms":1}\n'
    ;;
  overview)
    cat <<'JSON'
{"languages":[{"language":"javascript","files":1,"lines":100}],"entrypoints":[{"file":"index.js","symbol":"limit","role":"source","confidence":0.8,"reason":"fixture"}],"recommended_next_tools":[{"tool":"context_pack","priority":10,"reason":"fixture","suggested_arguments":{}}]}
JSON
    ;;
  context-pack)
    cat <<'JSON'
{"files":[{"file":"index.js","source":"seed_file","score":1.0,"reason":"Selected for high relevance via seed_file","ranges":[{"start_line":1,"end_line":5,"source":"seed_file","importance":"high"},{"start_line":6,"end_line":10,"source":"seed_file","importance":"high"},{"start_line":11,"end_line":15,"source":"seed_file","importance":"medium"}]}],"reading_plan":[{"file":"index.js","selection_rank":1,"focus":"Start with seed file context and primary symbols.","question":"What entrypoints, exported symbols, or setup code define the main flow here?","next_action":"inspect_seed_file","suggested_tool":{"tool":"file_outline","priority":10,"reason":"fixture","suggested_arguments":{"file":"index.js"}},"reason":"Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here?","selection_reason":"Selected for high relevance via seed_file"}],"estimated_tokens":120,"budget":{"applied_token_budget":6000,"omitted_files":0,"omitted_ranges":0,"truncation_reason":"none"},"continuation_summary":{"status":"complete","message":"complete","next_action":"read_selected_context","omitted_candidate_count":0},"omitted_candidates":[],"truncated":false}
JSON
    ;;
  *)
    echo "unexpected fake codeinsight invocation: $*" >&2
    exit 1
    ;;
esac
EOF

  chmod +x "$TEMP_DIR/bin/cargo" "$TEMP_DIR/bin/git" "$TEMP_DIR/bin/codeinsight"
}

run_benchmark() {
  local label="$1"
  local log_file="$TEMP_DIR/$label.log"
  local report_file="$TEMP_DIR/$label.md"
  local summary_file="$TEMP_DIR/$label.json"

  PATH="$TEMP_DIR/bin:$PATH" \
    CODEINSIGHT_FAKE_TARGET_DIR="$TEMP_DIR/target" \
    CODEINSIGHT_FAKE_GIT_LOG="$TEMP_DIR/git.log" \
    CODEINSIGHT_BIN="$TEMP_DIR/bin/codeinsight" \
    CODEINSIGHT_BENCH_WORKDIR="$TEMP_DIR/work" \
    CODEINSIGHT_BENCH_REUSE_REPOS=1 \
    CODEINSIGHT_BENCH_REPOS=p-limit \
    CODEINSIGHT_BENCH_OUTPUT="$report_file" \
    CODEINSIGHT_BENCH_SUMMARY_JSON="$summary_file" \
    "$ROOT_DIR/scripts/benchmark-smoke.sh" >"$log_file" 2>&1

  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" "$report_file" smoke p-limit >/dev/null
}

test_invalid_checkout_is_discarded() {
  local repo_dir="$TEMP_DIR/work/repos/p-limit"

  mkdir -p "$repo_dir/.git"
  : >"$TEMP_DIR/git.log"

  run_benchmark invalid

  grep -Fq "discarding invalid checkout for p-limit" "$TEMP_DIR/invalid.log" ||
    fail "invalid checkout was not reported"
  grep -Fq "clone	$repo_dir" "$TEMP_DIR/git.log" ||
    fail "invalid checkout did not trigger a fresh clone"
  [ -f "$repo_dir/index.js" ] ||
    fail "fresh clone did not restore the seed file"
}

test_valid_checkout_is_refreshed() {
  local repo_dir="$TEMP_DIR/work/repos/p-limit"

  rm -rf "$repo_dir"
  mkdir -p "$repo_dir/.git"
  touch "$repo_dir/.valid-git" "$repo_dir/dirty.tmp"
  : >"$TEMP_DIR/git.log"

  run_benchmark valid

  grep -Fq "reusing existing checkout for p-limit" "$TEMP_DIR/valid.log" ||
    fail "valid checkout was not reused"
  grep -Fq "reset	$repo_dir" "$TEMP_DIR/git.log" ||
    fail "valid checkout was not reset"
  grep -Fq "clean	$repo_dir" "$TEMP_DIR/git.log" ||
    fail "valid checkout was not cleaned"
  if grep -Fq "clone	$repo_dir" "$TEMP_DIR/git.log"; then
    fail "valid checkout unexpectedly cloned"
  fi
  [ ! -f "$repo_dir/dirty.tmp" ] ||
    fail "valid checkout clean did not remove dirty file"
  [ -f "$repo_dir/index.js" ] ||
    fail "valid checkout reset did not restore seed file"
}

main() {
  trap cleanup EXIT INT TERM
  write_fake_tools
  test_invalid_checkout_is_discarded
  test_valid_checkout_is_refreshed
  echo "benchmark reuse checkout smoke passed"
}

main "$@"
