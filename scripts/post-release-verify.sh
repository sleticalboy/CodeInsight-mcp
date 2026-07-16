#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY_RELEASE_SCRIPT="${CODEINSIGHT_VERIFY_RELEASE_SCRIPT:-$ROOT_DIR/scripts/verify-release.sh}"
UPDATE_RELEASE_STATUS_SCRIPT="${CODEINSIGHT_UPDATE_RELEASE_STATUS_SCRIPT:-$ROOT_DIR/scripts/update-release-status.sh}"
RAW_OUTPUT_FILE=""

cleanup() {
  if [ -n "$RAW_OUTPUT_FILE" ]; then
    rm -f "$RAW_OUTPUT_FILE"
  fi
}

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/post-release-verify.sh [options] <tag-or-version>

Runs post-release verification, saves the JSON summary, and refreshes the
generated release verification block in docs/status.md.

Options:
  --summary-file PATH       Write verify-release JSON to PATH.
  --status-doc PATH         Update PATH instead of docs/status.md.
  --evidence-json-file PATH Include archived pre-release evidence JSON in status.
  --evidence-file PATH      Include archived pre-release evidence Markdown in status.
  --skip-docker             Set CODEINSIGHT_SKIP_DOCKER=1.
  --skip-homebrew           Set CODEINSIGHT_SKIP_HOMEBREW=1.
  --skip-installed-quickstart
                            Set CODEINSIGHT_SKIP_INSTALLED_QUICKSTART=1.
  --allow-asset-download-unreachable
                            Set CODEINSIGHT_ALLOW_ASSET_DOWNLOAD_UNREACHABLE=1.
  -h, --help                Show this help.

Environment:
  CODEINSIGHT_STATUS_DATE=YYYY-MM-DD
  CODEINSIGHT_RELEASE_SUMMARY_DIR=release-verification
  CODEINSIGHT_VERIFY_RELEASE_SCRIPT=scripts/verify-release.sh
  CODEINSIGHT_UPDATE_RELEASE_STATUS_SCRIPT=scripts/update-release-status.sh
EOF
  exit "$status"
}

normalize_tag() {
  case "$1" in
    v*) printf '%s' "$1" ;;
    *) printf 'v%s' "$1" ;;
  esac
}

extract_summary_json() {
  local raw_output="$1"
  local summary_file="$2"

  ruby -rjson - "$raw_output" "$summary_file" <<'RUBY'
raw_path, summary_path = ARGV
text = File.read(raw_path)
objects = []
start_index = nil
depth = 0
in_string = false
escape = false

text.each_char.with_index do |char, index|
  if in_string
    if escape
      escape = false
    elsif char == "\\"
      escape = true
    elsif char == '"'
      in_string = false
    end
    next
  end

  case char
  when '"'
    in_string = true if depth.positive?
  when "{"
    start_index = index if depth.zero?
    depth += 1
  when "}"
    next if depth.zero?

    depth -= 1
    if depth.zero? && start_index
      candidate = text[start_index..index]
      begin
        parsed = JSON.parse(candidate)
        objects << parsed if parsed.is_a?(Hash)
      rescue JSON::ParserError
        # Ignore non-summary JSON-looking output.
      end
      start_index = nil
    end
  end
end

summary = objects.reverse.find do |object|
  object["status"] == "passed" &&
    object["tag"].is_a?(String) &&
    object["version"].is_a?(String) &&
    object["gates"].is_a?(Hash)
end

abort("release verification summary JSON not found in #{raw_path}") unless summary

File.write(summary_path, JSON.pretty_generate(summary) + "\n")
RUBY
}

main() {
  local tag_input=""
  local summary_file=""
  local status_doc="$ROOT_DIR/docs/status.md"
  local evidence_json_file=""
  local evidence_file=""
  local skip_docker=0
  local skip_homebrew=0
  local skip_installed_quickstart=0
  local allow_asset_download_unreachable=0

  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h | --help)
        usage 0
        ;;
      --summary-file)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        summary_file="$1"
        ;;
      --status-doc)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        status_doc="$1"
        ;;
      --evidence-file)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        evidence_file="$1"
        ;;
      --evidence-json-file)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        evidence_json_file="$1"
        ;;
      --skip-docker)
        skip_docker=1
        ;;
      --skip-homebrew)
        skip_homebrew=1
        ;;
      --skip-installed-quickstart)
        skip_installed_quickstart=1
        ;;
      --allow-asset-download-unreachable)
        allow_asset_download_unreachable=1
        ;;
      --)
        shift
        break
        ;;
      -*)
        usage
        ;;
      *)
        if [ -n "$tag_input" ]; then
          usage
        fi
        tag_input="$1"
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    if [ -n "$tag_input" ]; then
      usage
    fi
    tag_input="$1"
    shift
  done

  if [ -z "$tag_input" ]; then
    usage
  fi

  local tag summary_dir
  tag="$(normalize_tag "$tag_input")"
  if [ -z "$summary_file" ]; then
    summary_dir="${CODEINSIGHT_RELEASE_SUMMARY_DIR:-$ROOT_DIR/release-verification}"
    summary_file="$summary_dir/${tag}.json"
  fi
  if [ -z "$evidence_json_file" ] && [ -z "$evidence_file" ] && [ -f "$ROOT_DIR/release-evidence/${tag}.json" ]; then
    evidence_json_file="$ROOT_DIR/release-evidence/${tag}.json"
  fi
  if [ -z "$evidence_json_file" ] && [ -z "$evidence_file" ] && [ -f "$ROOT_DIR/release-evidence/${tag}.md" ]; then
    evidence_file="$ROOT_DIR/release-evidence/${tag}.md"
  fi
  mkdir -p "$(dirname "$summary_file")"
  RAW_OUTPUT_FILE="$(mktemp)"
  trap cleanup EXIT INT TERM

  local -a verify_env=()
  if [ "$skip_docker" -eq 1 ]; then
    verify_env+=(CODEINSIGHT_SKIP_DOCKER=1)
  fi
  if [ "$skip_homebrew" -eq 1 ]; then
    verify_env+=(CODEINSIGHT_SKIP_HOMEBREW=1)
  fi
  if [ "$skip_installed_quickstart" -eq 1 ]; then
    verify_env+=(CODEINSIGHT_SKIP_INSTALLED_QUICKSTART=1)
  fi
  if [ "$allow_asset_download_unreachable" -eq 1 ]; then
    verify_env+=(CODEINSIGHT_ALLOW_ASSET_DOWNLOAD_UNREACHABLE=1)
  fi

  echo "running release verification for ${tag}"
  env "${verify_env[@]}" "$VERIFY_RELEASE_SCRIPT" --json "$tag" | tee "$RAW_OUTPUT_FILE"
  extract_summary_json "$RAW_OUTPUT_FILE" "$summary_file"

  echo "updating status document"
  local -a update_args=()
  if [ -n "$evidence_json_file" ]; then
    update_args+=(--evidence-json-file "$evidence_json_file")
  elif [ -n "$evidence_file" ]; then
    update_args+=(--evidence-file "$evidence_file")
  fi
  update_args+=("$summary_file" "$status_doc")
  "$UPDATE_RELEASE_STATUS_SCRIPT" "${update_args[@]}"

  echo "post-release verification passed"
  echo "summary: $summary_file"
  echo "status: $status_doc"
  if [ -n "$evidence_json_file" ]; then
    echo "evidence_json: $evidence_json_file"
  elif [ -n "$evidence_file" ]; then
    echo "evidence: $evidence_file"
  fi
}

main "$@"
