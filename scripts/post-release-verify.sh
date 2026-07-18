#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY_RELEASE_SCRIPT="${CODEINSIGHT_VERIFY_RELEASE_SCRIPT:-$ROOT_DIR/scripts/verify-release.sh}"
UPDATE_RELEASE_STATUS_SCRIPT="${CODEINSIGHT_UPDATE_RELEASE_STATUS_SCRIPT:-$ROOT_DIR/scripts/update-release-status.sh}"
RELEASE_HANDOFF_SUMMARY_SCRIPT="${CODEINSIGHT_RELEASE_HANDOFF_SUMMARY_SCRIPT:-$ROOT_DIR/scripts/release-handoff-summary.sh}"
RELEASE_EVIDENCE_SUMMARY_SCRIPT="${CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT:-$ROOT_DIR/scripts/release-evidence-summary.sh}"
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
  --handoff                 Write release handoff Markdown and JSON.
  --handoff-output PATH     Write release handoff Markdown to PATH.
  --handoff-json-output PATH
                            Write release handoff JSON to PATH.
  --generate-evidence-for-handoff
                            Generate release-evidence/<tag>.json before handoff
                            when the JSON archive is missing.
  --evidence-branch BRANCH  Branch passed when generating handoff evidence.
                            Default: main.
  --evidence-head-sha SHA   Head SHA passed when generating handoff evidence.
  --evidence-run-id ID      CI run ID passed when generating handoff evidence.
  --repo OWNER/REPO         Repository passed when generating handoff evidence.
  --release-evidence-summary-script PATH
                            Evidence summary script used by handoff generation.
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
  CODEINSIGHT_RELEASE_HANDOFF_DIR=release-handoff
  CODEINSIGHT_VERIFY_RELEASE_SCRIPT=scripts/verify-release.sh
  CODEINSIGHT_UPDATE_RELEASE_STATUS_SCRIPT=scripts/update-release-status.sh
  CODEINSIGHT_RELEASE_HANDOFF_SUMMARY_SCRIPT=scripts/release-handoff-summary.sh
  CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT=scripts/release-evidence-summary.sh
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
  local handoff_enabled=0
  local handoff_output_file=""
  local handoff_json_output_file=""
  local handoff_generate_evidence=0
  local handoff_should_generate_evidence=0
  local handoff_evidence_branch="main"
  local handoff_evidence_head_sha=""
  local handoff_evidence_run_id=""
  local skip_docker=0
  local skip_homebrew=0
  local skip_installed_quickstart=0
  local allow_asset_download_unreachable=0
  local -a handoff_evidence_repo_arg=()

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
      --handoff)
        handoff_enabled=1
        ;;
      --handoff-output)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        handoff_enabled=1
        handoff_output_file="$1"
        ;;
      --handoff-json-output)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        handoff_enabled=1
        handoff_json_output_file="$1"
        ;;
      --generate-evidence-for-handoff)
        handoff_enabled=1
        handoff_generate_evidence=1
        ;;
      --evidence-branch)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        handoff_evidence_branch="$1"
        ;;
      --evidence-head-sha)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        handoff_evidence_head_sha="$1"
        ;;
      --evidence-run-id)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        handoff_evidence_run_id="$1"
        ;;
      --repo)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        handoff_evidence_repo_arg=(--repo "$1")
        ;;
      --release-evidence-summary-script)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        RELEASE_EVIDENCE_SUMMARY_SCRIPT="$1"
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
  if [ "$handoff_enabled" -eq 1 ] && [ -z "$evidence_json_file" ]; then
    if [ "$handoff_generate_evidence" -eq 1 ]; then
      evidence_json_file="$ROOT_DIR/release-evidence/${tag}.json"
      evidence_file=""
      handoff_should_generate_evidence=1
    else
      echo "post-release verification failed: release handoff requires --evidence-json-file, release-evidence/${tag}.json, or --generate-evidence-for-handoff" >&2
      exit 1
    fi
  fi
  if [ "$handoff_enabled" -eq 1 ] && [ "$handoff_generate_evidence" -eq 1 ] && [ -n "$evidence_json_file" ] && [ ! -f "$evidence_json_file" ]; then
    handoff_should_generate_evidence=1
  fi
  if [ "$handoff_enabled" -eq 1 ]; then
    local handoff_dir
    handoff_dir="${CODEINSIGHT_RELEASE_HANDOFF_DIR:-$ROOT_DIR/release-handoff}"
    if [ -z "$handoff_output_file" ]; then
      handoff_output_file="$handoff_dir/${tag}.md"
    fi
    if [ -z "$handoff_json_output_file" ]; then
      handoff_json_output_file="$handoff_dir/${tag}.json"
    fi
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

  local handoff_written=0
  if [ "$handoff_enabled" -eq 1 ] && [ "$handoff_should_generate_evidence" -eq 1 ]; then
    echo "writing release handoff"
    local -a handoff_generate_args=(
      --generate-evidence
      --evidence-json "$evidence_json_file"
      --evidence-branch "$handoff_evidence_branch"
      --release-evidence-summary-script "$RELEASE_EVIDENCE_SUMMARY_SCRIPT"
      --verification-json "$summary_file"
      --json-output "$handoff_json_output_file"
      --output "$handoff_output_file"
    )
    if [ "${#handoff_evidence_repo_arg[@]}" -gt 0 ]; then
      handoff_generate_args+=("${handoff_evidence_repo_arg[@]}")
    fi
    if [ -n "$handoff_evidence_head_sha" ]; then
      handoff_generate_args+=(--evidence-head-sha "$handoff_evidence_head_sha")
    fi
    if [ -n "$handoff_evidence_run_id" ]; then
      handoff_generate_args+=(--evidence-run-id "$handoff_evidence_run_id")
    fi
    "$RELEASE_HANDOFF_SUMMARY_SCRIPT" "${handoff_generate_args[@]}" "$tag"
    handoff_written=1
  fi

  echo "updating status document"
  local -a update_args=()
  if [ -n "$evidence_json_file" ]; then
    update_args+=(--evidence-json-file "$evidence_json_file")
  elif [ -n "$evidence_file" ]; then
    update_args+=(--evidence-file "$evidence_file")
  fi
  update_args+=("$summary_file" "$status_doc")
  "$UPDATE_RELEASE_STATUS_SCRIPT" "${update_args[@]}"

  if [ "$handoff_enabled" -eq 1 ] && [ "$handoff_written" -eq 0 ]; then
    echo "writing release handoff"
    "$RELEASE_HANDOFF_SUMMARY_SCRIPT" \
      --evidence-json "$evidence_json_file" \
      --verification-json "$summary_file" \
      --json-output "$handoff_json_output_file" \
      --output "$handoff_output_file" \
      "$tag"
  fi

  echo "post-release verification passed"
  echo "summary: $summary_file"
  echo "status: $status_doc"
  if [ -n "$evidence_json_file" ]; then
    echo "evidence_json: $evidence_json_file"
  elif [ -n "$evidence_file" ]; then
    echo "evidence: $evidence_file"
  fi
  if [ "$handoff_enabled" -eq 1 ]; then
    echo "handoff_json: $handoff_json_output_file"
    echo "handoff: $handoff_output_file"
  fi
}

main "$@"
