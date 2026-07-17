#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
TAG_NAME=""
EVIDENCE_JSON_FILE=""
VERIFICATION_JSON_FILE=""
JSON_OUTPUT_FILE=""
OUTPUT_FILE=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-handoff-summary.sh [options] <tag>

Combines pre-release evidence JSON and post-release verification JSON into a
single release handoff summary. Markdown is printed to stdout by default.

Options:
  --evidence-json PATH      Read pre-release evidence JSON from PATH.
                            Default: release-evidence/<tag>.json.
  --verification-json PATH  Read post-release verification JSON from PATH.
                            Default: release-verification/<tag>.json.
  --json-output PATH        Write a machine-readable handoff JSON to PATH.
  --output PATH             Write the Markdown handoff summary to PATH.
  -h, --help                Show this help.

Environment:
  CODEINSIGHT_ROOT_DIR=/path/to/repo
EOF
  exit "$status"
}

fail() {
  echo "release handoff summary failed: $*" >&2
  exit 1
}

normalize_tag() {
  local tag="$1"
  case "$tag" in
    v*) printf "%s" "$tag" ;;
    *) printf "v%s" "$tag" ;;
  esac
}

main() {
  local markdown_file

  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h | --help)
        usage 0
        ;;
      --evidence-json)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EVIDENCE_JSON_FILE="$1"
        ;;
      --verification-json)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        VERIFICATION_JSON_FILE="$1"
        ;;
      --json-output)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        JSON_OUTPUT_FILE="$1"
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
        if [ -n "$TAG_NAME" ]; then
          usage
        fi
        TAG_NAME="$(normalize_tag "$1")"
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    if [ -n "$TAG_NAME" ]; then
      usage
    fi
    TAG_NAME="$(normalize_tag "$1")"
    shift
  done

  if [ -z "$TAG_NAME" ]; then
    usage
  fi
  if [[ ! "$TAG_NAME" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fail "tag must look like vX.Y.Z or X.Y.Z: $TAG_NAME"
  fi
  if [ -z "$EVIDENCE_JSON_FILE" ]; then
    EVIDENCE_JSON_FILE="$ROOT_DIR/release-evidence/$TAG_NAME.json"
  fi
  if [ -z "$VERIFICATION_JSON_FILE" ]; then
    VERIFICATION_JSON_FILE="$ROOT_DIR/release-verification/$TAG_NAME.json"
  fi
  if [ ! -f "$EVIDENCE_JSON_FILE" ]; then
    fail "evidence JSON not found: $EVIDENCE_JSON_FILE"
  fi
  if [ ! -f "$VERIFICATION_JSON_FILE" ]; then
    fail "verification JSON not found: $VERIFICATION_JSON_FILE"
  fi

  markdown_file="$(mktemp)"
  trap "rm -f '$markdown_file'" EXIT INT TERM

  TAG_NAME="$TAG_NAME" \
    EVIDENCE_JSON_FILE="$EVIDENCE_JSON_FILE" \
    VERIFICATION_JSON_FILE="$VERIFICATION_JSON_FILE" \
    JSON_OUTPUT_FILE="$JSON_OUTPUT_FILE" \
    ruby -rjson -rfileutils - "$markdown_file" <<'RUBY'
markdown_path = ARGV.fetch(0)
tag = ENV.fetch("TAG_NAME")
evidence_path = ENV.fetch("EVIDENCE_JSON_FILE")
verification_path = ENV.fetch("VERIFICATION_JSON_FILE")
json_output_path = ENV.fetch("JSON_OUTPUT_FILE")

evidence = JSON.parse(File.read(evidence_path))
verification = JSON.parse(File.read(verification_path))

def fail!(message)
  warn("release handoff summary failed: #{message}")
  exit(1)
end

fail!("evidence schema_version must be 1") unless evidence["schema_version"] == 1
fail!("evidence tag #{evidence["tag"]} does not match #{tag}") unless evidence["tag"] == tag
fail!("verification tag #{verification["tag"]} does not match #{tag}") unless verification["tag"] == tag
fail!("verification status must be passed") unless verification["status"] == "passed"

metadata = evidence.fetch("metadata")
ci = evidence.fetch("ci")
artifacts = evidence.fetch("artifacts")
benchmark = artifacts.fetch("benchmark")
benchmark_metrics = benchmark["metrics"] || {}
quality = artifacts.fetch("context_pack_quality")
agent_route = artifacts.fetch("agent_route")
mcp_first_call = artifacts.fetch("mcp_first_call")
gates = verification.fetch("gates")
expected_assets = verification.fetch("expected_assets")

markdown_lines = [
  "## #{tag} release handoff",
  "",
  "- Status: `#{verification.fetch("status")}`",
  "- Version: `#{verification.fetch("version")}`",
  "- Repository: `#{verification.fetch("repo")}`",
  "- Target commit: `#{evidence.fetch("head_sha")}`",
  "- Pre-release CI: [run #{ci.fetch("run_id")}](#{ci.fetch("url")})",
  "- Evidence JSON: `#{evidence_path}`",
  "- Verification JSON: `#{verification_path}`",
  "- Metadata: `cargo=#{metadata.fetch("cargo")}`, `install=#{metadata.fetch("install")}`, `changelog=#{metadata.fetch("changelog")}`",
  "",
  "### Release Gates",
  "",
  *gates.map { |key, value| "- `#{key}`: `#{value}`" },
  "",
  "### Expected Assets",
  "",
  *expected_assets.map { |asset| "- `#{asset}`" },
  "",
  "### Pre-release Artifacts",
  "",
  "- Benchmark artifact: [#{benchmark.fetch("name")}](#{benchmark.fetch("url")})",
  ("- Benchmark routing: `context_pack` first for #{benchmark_metrics.fetch("context_pack_first")}/#{benchmark_metrics.fetch("routing_total")} repositories" if benchmark_metrics.key?("context_pack_first") && benchmark_metrics.key?("routing_total")),
  ("- Benchmark line reduction: `#{benchmark_metrics.fetch("line_reduction")}`" if benchmark_metrics.key?("line_reduction")),
  ("- Benchmark guardrail failures: `#{benchmark_metrics.fetch("guardrail_failures")}`" if benchmark_metrics.key?("guardrail_failures")),
  ("- Benchmark truncated context packs: `#{benchmark_metrics.fetch("truncated_packs")}`" if benchmark_metrics.key?("truncated_packs")),
  "- Context-pack quality artifact: [#{quality.fetch("name")}](#{quality.fetch("url")})",
  "- Agent-route artifact: [#{agent_route.fetch("name")}](#{agent_route.fetch("url")})",
  "- MCP first-call artifact: [#{mcp_first_call.fetch("name")}](#{mcp_first_call.fetch("url")})"
].compact

markdown = markdown_lines.join("\n")
File.write(markdown_path, "#{markdown}\n")

handoff = {
  "schema_version" => 1,
  "tag" => tag,
  "status" => verification.fetch("status"),
  "version" => verification.fetch("version"),
  "repo" => verification.fetch("repo"),
  "target_commit" => evidence.fetch("head_sha"),
  "evidence_json" => evidence_path,
  "verification_json" => verification_path,
  "pre_release" => {
    "branch" => evidence.fetch("branch"),
    "ci" => ci,
    "metadata" => metadata,
    "artifacts" => artifacts
  },
  "post_release" => {
    "gates" => gates,
    "expected_assets" => expected_assets,
    "docker" => verification["docker"],
    "homebrew" => verification["homebrew"],
    "installed_quickstart" => verification["installed_quickstart"]
  },
  "handoff_markdown" => markdown
}

unless json_output_path.empty?
  FileUtils.mkdir_p(File.dirname(json_output_path))
  File.write(json_output_path, "#{JSON.pretty_generate(handoff)}\n")
end
RUBY

  if [ -n "$OUTPUT_FILE" ]; then
    mkdir -p "$(dirname "$OUTPUT_FILE")"
    cp "$markdown_file" "$OUTPUT_FILE"
    echo "release handoff summary written: $OUTPUT_FILE"
  else
    cat "$markdown_file"
  fi
  if [ -n "$JSON_OUTPUT_FILE" ]; then
    echo "release handoff JSON written: $JSON_OUTPUT_FILE"
  fi
}

main "$@"
