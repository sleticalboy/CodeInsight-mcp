#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
INPUT=""
CHANGELOG_NOTES_FILE=""
OUTPUT_FILE=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-notes-draft.sh [options] <handoff-json-or-tag>

Builds a GitHub Release/status-PR notes draft from release handoff JSON.
Pass a tag such as vX.Y.Z to read release-handoff/<tag>.json.

Options:
  --changelog-notes PATH  Prepend existing changelog-derived release notes.
  --output PATH           Write the Markdown draft to PATH instead of stdout.
  -h, --help              Show this help.

Environment:
  CODEINSIGHT_ROOT_DIR=/path/to/repo
EOF
  exit "$status"
}

fail() {
  echo "release notes draft failed: $*" >&2
  exit 1
}

normalize_tag() {
  case "$1" in
    v*) printf "%s" "$1" ;;
    *) printf "v%s" "$1" ;;
  esac
}

resolve_input() {
  local input="$1"

  if [ -f "$input" ]; then
    printf "%s" "$input"
    return
  fi

  local tag
  tag="$(normalize_tag "$input")"
  if [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    printf "%s" "$ROOT_DIR/release-handoff/$tag.json"
    return
  fi

  printf "%s" "$input"
}

main() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h | --help)
        usage 0
        ;;
      --changelog-notes)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        CHANGELOG_NOTES_FILE="$1"
        ;;
      --output)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        OUTPUT_FILE="$1"
        ;;
      --)
        shift
        break
        ;;
      -*)
        usage
        ;;
      *)
        if [ -n "$INPUT" ]; then
          usage
        fi
        INPUT="$1"
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    if [ -n "$INPUT" ]; then
      usage
    fi
    INPUT="$1"
    shift
  done

  if [ -z "$INPUT" ]; then
    usage
  fi

  local handoff_json
  handoff_json="$(resolve_input "$INPUT")"
  if [ ! -f "$handoff_json" ]; then
    fail "handoff JSON not found: $handoff_json"
  fi
  if [ -n "$CHANGELOG_NOTES_FILE" ] && [ ! -f "$CHANGELOG_NOTES_FILE" ]; then
    fail "changelog notes not found: $CHANGELOG_NOTES_FILE"
  fi

  local output_target="/dev/stdout"
  if [ -n "$OUTPUT_FILE" ]; then
    mkdir -p "$(dirname "$OUTPUT_FILE")"
    output_target="$OUTPUT_FILE"
  fi

  HANDOFF_JSON_FILE="$handoff_json" \
    CHANGELOG_NOTES_FILE="$CHANGELOG_NOTES_FILE" \
    ruby -rjson - "$output_target" <<'RUBY'
output_path = ARGV.fetch(0)
handoff_path = ENV.fetch("HANDOFF_JSON_FILE")
changelog_notes_path = ENV.fetch("CHANGELOG_NOTES_FILE")

def fail!(message)
  warn("release notes draft failed: #{message}")
  exit(1)
end

handoff = JSON.parse(File.read(handoff_path))

fail!("handoff schema_version must be 1") unless handoff["schema_version"] == 1
fail!("handoff status must be passed") unless handoff["status"] == "passed"

tag = handoff.fetch("tag")
version = handoff.fetch("version")
repo = handoff.fetch("repo")
target_commit = handoff.fetch("target_commit")
pre_release = handoff.fetch("pre_release")
post_release = handoff.fetch("post_release")
ci = pre_release.fetch("ci")
metadata = pre_release.fetch("metadata")
artifacts = pre_release.fetch("artifacts")
gates = post_release.fetch("gates")
expected_assets = post_release.fetch("expected_assets")
docker = post_release["docker"] || {}
homebrew = post_release["homebrew"] || {}
installed_quickstart = post_release["installed_quickstart"] || {}

lines = []
lines << "## #{tag} release notes draft"
lines << ""

unless changelog_notes_path.empty?
  changelog_notes = File.read(changelog_notes_path).strip
  unless changelog_notes.empty?
    lines << changelog_notes
    lines << ""
  end
end

lines << "### Verification Evidence"
lines << ""
lines << "- Status: `#{handoff.fetch("status")}`"
lines << "- Version: `#{version}`"
lines << "- Repository: `#{repo}`"
lines << "- Target commit: `#{target_commit}`"
lines << "- Pre-release CI: [run #{ci.fetch("run_id")}](#{ci.fetch("url")})"
lines << "- Metadata: `cargo=#{metadata.fetch("cargo")}`, `install=#{metadata.fetch("install")}`, `changelog=#{metadata.fetch("changelog")}`"
lines << ""
lines << "### Release Gates"
lines << ""
gates.each do |key, value|
  lines << "- `#{key}`: `#{value}`"
end
lines << ""
lines << "### Expected Assets"
lines << ""
expected_assets.each do |asset|
  lines << "- `#{asset}`"
end
lines << ""
lines << "### Distribution Checks"
lines << ""
lines << "- Docker image: `#{docker["image"] || "-"}` (#{docker["skipped"] ? "skipped locally" : "verified"})"
lines << "- Homebrew tap: `#{homebrew["tap"] || "-"}` (#{homebrew["skipped"] ? "skipped locally" : "verified"})"
lines << "- Installed quickstart binary: `#{installed_quickstart["binary"] || "-"}` (#{installed_quickstart["skipped"] ? "skipped locally" : "verified"})"

coverage = installed_quickstart["coverage"]
if coverage.is_a?(Array) && !coverage.empty?
  lines << "- Installed quickstart coverage: `#{coverage.join("`, `")}`"
end

lines << ""
lines << "### Pre-release Artifacts"
lines << ""
[
  ["Benchmark", artifacts.fetch("benchmark")],
  ["Context-pack quality", artifacts.fetch("context_pack_quality")],
  ["Agent-route", artifacts.fetch("agent_route")]
].each do |label, artifact|
  lines << "- #{label}: [#{artifact.fetch("name")}](#{artifact.fetch("url")})"
end

File.write(output_path, "#{lines.join("\n")}\n")
RUBY

  if [ -n "$OUTPUT_FILE" ]; then
    echo "release notes draft written: $OUTPUT_FILE"
  fi
}

main "$@"
