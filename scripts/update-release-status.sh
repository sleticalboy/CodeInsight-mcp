#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLOCK_FILE=""

cleanup() {
  if [ -n "$BLOCK_FILE" ]; then
    rm -f "$BLOCK_FILE"
  fi
}

usage() {
  cat >&2 <<'EOF'
usage: scripts/update-release-status.sh <verify-release-summary.json> [status-doc]

Updates the generated release verification summary block in docs/status.md.

Environment:
  CODEINSIGHT_STATUS_DATE=YYYY-MM-DD
EOF
  exit 2
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

status_label() {
  case "$1" in
    passed) printf 'passed' ;;
    skipped) printf 'skipped' ;;
    metadata_only) printf 'metadata-only' ;;
    *) printf '%s' "$1" ;;
  esac
}

main() {
  if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    usage
  fi

  require_command jq
  require_command ruby

  local summary_file="$1"
  local status_file="${2:-$ROOT_DIR/docs/status.md}"
  local generated_date="${CODEINSIGHT_STATUS_DATE:-$(date +%F)}"

  if [ ! -f "$summary_file" ]; then
    echo "summary JSON not found: $summary_file" >&2
    exit 1
  fi
  if [ ! -f "$status_file" ]; then
    echo "status document not found: $status_file" >&2
    exit 1
  fi

  jq -e '
    .status == "passed" and
    (.tag | type == "string" and length > 0) and
    (.repo | type == "string" and length > 0) and
    (.version | type == "string" and length > 0) and
    (.gates | type == "object") and
    (.expected_assets | type == "array")
  ' "$summary_file" >/dev/null

  BLOCK_FILE="$(mktemp)"
  trap cleanup EXIT INT TERM

  {
    echo "<!-- release-verification-summary:start -->"
    echo "### Release Verification Summary"
    echo
    printf 'Generated from `scripts/verify-release.sh --json` on %s.\n' "$generated_date"
    echo
    printf -- '- Status: `%s`\n' "$(jq -r '.status' "$summary_file")"
    printf -- '- Tag: `%s`\n' "$(jq -r '.tag' "$summary_file")"
    printf -- '- Version: `%s`\n' "$(jq -r '.version' "$summary_file")"
    printf -- '- Repository: `%s`\n' "$(jq -r '.repo' "$summary_file")"
    echo "- Gates:"
    jq -r '.gates | to_entries[] | [.key, .value] | @tsv' "$summary_file" |
      while IFS="$(printf '\t')" read -r gate status; do
        printf '  - `%s`: `%s`\n' "$gate" "$(status_label "$status")"
      done
    echo "- Expected release assets:"
    jq -r '.expected_assets[]' "$summary_file" |
      while IFS= read -r asset; do
        printf '  - `%s`\n' "$asset"
      done
    printf -- '- Docker image: `%s` (%s)\n' \
      "$(jq -r '.docker.image // "-"' "$summary_file")" \
      "$(if [ "$(jq -r '.docker.skipped // false' "$summary_file")" = "true" ]; then printf 'skipped locally'; else printf 'verified'; fi)"
    printf -- '- Homebrew tap: `%s` (%s)\n' \
      "$(jq -r '.homebrew.tap // "-"' "$summary_file")" \
      "$(if [ "$(jq -r '.homebrew.skipped // false' "$summary_file")" = "true" ]; then printf 'skipped locally'; else printf 'verified'; fi)"
    printf -- '- Installed quickstart binary: `%s` (%s)\n' \
      "$(jq -r '.installed_quickstart.binary // "-"' "$summary_file")" \
      "$(if [ "$(jq -r '.installed_quickstart.skipped // false' "$summary_file")" = "true" ]; then printf 'skipped locally'; else printf 'verified'; fi)"
    echo "<!-- release-verification-summary:end -->"
  } >"$BLOCK_FILE"

  ruby - "$status_file" "$BLOCK_FILE" <<'RUBY'
status_path, block_path = ARGV
status = File.read(status_path)
block = File.read(block_path).strip
start_marker = "<!-- release-verification-summary:start -->"
end_marker = "<!-- release-verification-summary:end -->"

if status.include?(start_marker) && status.include?(end_marker)
  pattern = /#{Regexp.escape(start_marker)}.*?#{Regexp.escape(end_marker)}\n*/m
  updated = status.sub(pattern, "#{block}\n\n")
else
  heading = /^## Latest Verified Release\n/
  match = status.match(heading)
  abort("Latest Verified Release section not found in #{status_path}") unless match

  insert_at = status.index(/^## /, match.end(0)) || status.length
  updated = status.dup
  updated.insert(insert_at, "\n#{block}\n\n")
end

File.write(status_path, updated)
RUBY

  printf 'updated release verification summary in %s\n' "$status_file"
}

main "$@"
