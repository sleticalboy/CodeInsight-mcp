#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
RELEASE_EVIDENCE_SUMMARY_SCRIPT="${CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT:-$ROOT_DIR/scripts/release-evidence-summary.sh}"
TAG_NAME=""
EVIDENCE_JSON_FILE=""
VERIFICATION_JSON_FILE=""
JSON_OUTPUT_FILE=""
OUTPUT_FILE=""
GENERATE_EVIDENCE=0
EVIDENCE_BRANCH="main"
EVIDENCE_HEAD_SHA=""
EVIDENCE_RUN_ID=""
EVIDENCE_REPO_ARG=()

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
  --generate-evidence       Generate the evidence JSON first with
                            scripts/release-evidence-summary.sh --json-output.
  --evidence-branch BRANCH  Branch passed when generating evidence. Default: main.
  --evidence-head-sha SHA   Head SHA passed when generating evidence.
  --evidence-run-id ID      CI run ID passed when generating evidence.
  --repo OWNER/REPO         Repository passed when generating evidence.
  --release-evidence-summary-script PATH
                            Evidence summary script. Default:
                            scripts/release-evidence-summary.sh.
  --verification-json PATH  Read post-release verification JSON from PATH.
                            Default: release-verification/<tag>.json.
  --json-output PATH        Write a machine-readable handoff JSON to PATH.
  --output PATH             Write the Markdown handoff summary to PATH.
  -h, --help                Show this help.

Environment:
  CODEINSIGHT_ROOT_DIR=/path/to/repo
  CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT=scripts/release-evidence-summary.sh
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

generate_evidence_json() {
  local evidence_json_file="$1"
  local evidence_markdown_file
  local temp_json
  local temp_markdown
  local -a args

  if [ ! -x "$RELEASE_EVIDENCE_SUMMARY_SCRIPT" ]; then
    fail "release evidence summary script is not executable: $RELEASE_EVIDENCE_SUMMARY_SCRIPT"
  fi

  evidence_markdown_file="${evidence_json_file%.json}.md"
  temp_json="$(mktemp)"
  temp_markdown="$(mktemp)"
  mkdir -p "$(dirname "$evidence_json_file")" "$(dirname "$evidence_markdown_file")"

  args=()
  if [ "${#EVIDENCE_REPO_ARG[@]}" -gt 0 ]; then
    args+=("${EVIDENCE_REPO_ARG[@]}")
  fi
  if [ -n "$EVIDENCE_RUN_ID" ]; then
    args+=(--run-id "$EVIDENCE_RUN_ID")
  fi
  if [ -n "$EVIDENCE_HEAD_SHA" ]; then
    args+=(--head-sha "$EVIDENCE_HEAD_SHA")
  fi
  args+=(--json-output "$temp_json" "$TAG_NAME" "$EVIDENCE_BRANCH")

  if ! CODEINSIGHT_ROOT_DIR="$ROOT_DIR" "$RELEASE_EVIDENCE_SUMMARY_SCRIPT" "${args[@]}" >"$temp_markdown"; then
    rm -f "$temp_json" "$temp_markdown"
    fail "could not generate release evidence JSON"
  fi

  mv "$temp_json" "$evidence_json_file"
  mv "$temp_markdown" "$evidence_markdown_file"
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
      --generate-evidence)
        GENERATE_EVIDENCE=1
        ;;
      --evidence-branch)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EVIDENCE_BRANCH="$1"
        ;;
      --evidence-head-sha)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EVIDENCE_HEAD_SHA="$1"
        ;;
      --evidence-run-id)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EVIDENCE_RUN_ID="$1"
        ;;
      --repo)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EVIDENCE_REPO_ARG=(--repo "$1")
        ;;
      --release-evidence-summary-script)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        RELEASE_EVIDENCE_SUMMARY_SCRIPT="$1"
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
  if [ "$GENERATE_EVIDENCE" -eq 1 ]; then
    generate_evidence_json "$EVIDENCE_JSON_FILE"
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
adoption_report = artifacts["adoption_report"]
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
  "- MCP first-call artifact: [#{mcp_first_call.fetch("name")}](#{mcp_first_call.fetch("url")})",
  (if adoption_report
     "- Adoption report: [#{adoption_report.fetch("name")}](#{adoption_report.fetch("document")})"
   end),
  (if adoption_report
     "- Adoption report command: `#{adoption_report.fetch("command")}`"
   end),
  (if adoption_report
     "- Adoption report archive: `#{adoption_report.fetch("archive")}`"
   end),
  (if adoption_report
     metrics = adoption_report.fetch("metrics")
     "- Adoption report routed first-read: `#{metrics.fetch("selected_lines")}/#{metrics.fetch("total_lines")}` source lines, `#{metrics.fetch("line_reduction")}` reduction"
   end),
  (if adoption_report
     metrics = adoption_report.fetch("metrics")
     contract = metrics.fetch("mcp_first_call_contract")
     "- Adoption report MCP first-call contract: `reading_order=#{contract.fetch("reading_order")}`, `suggested_tool_handoff=#{contract.fetch("suggested_tool_handoff")}`, `continuation_after_selected_context=#{contract.fetch("continuation_after_selected_context")}`, `suggested_tool_executed=#{contract.fetch("suggested_tool_executed")}`"
   end)
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
