#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local summary_file="$TEMP_DIR/summary.json"
  local status_doc="$TEMP_DIR/status.md"
  local evidence_json_file="$TEMP_DIR/release-evidence/v9.8.7.json"
  local handoff_file="$TEMP_DIR/release-handoff/v9.8.7.md"
  local handoff_json_file="$TEMP_DIR/release-handoff/v9.8.7.json"
  local generated_summary_file="$TEMP_DIR/generated-summary.json"
  local generated_status_doc="$TEMP_DIR/generated-status.md"
  local generated_evidence_json_file="$TEMP_DIR/generated-release-evidence/v9.8.7.json"
  local generated_handoff_file="$TEMP_DIR/generated-release-handoff/v9.8.7.md"
  local generated_handoff_json_file="$TEMP_DIR/generated-release-handoff/v9.8.7.json"
  local fake_verify="$TEMP_DIR/verify-release.sh"
  local fake_update="$TEMP_DIR/update-release-status.sh"
  local fake_handoff="$TEMP_DIR/release-handoff-summary.sh"
  local fake_release_evidence_summary="$TEMP_DIR/release-evidence-summary.sh"

  cat >"$fake_verify" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

test "${CODEINSIGHT_SKIP_DOCKER:-}" = "1"
test "${CODEINSIGHT_SKIP_HOMEBREW:-}" = "1"
test "${CODEINSIGHT_SKIP_INSTALLED_QUICKSTART:-}" = "1"
test "${CODEINSIGHT_ALLOW_ASSET_DOWNLOAD_UNREACHABLE:-}" = "1"
test "$1" = "--json"
test "$2" = "v9.8.7"

cat <<'LOG'
==> Verify public install script
{"name":"codeinsight","version":"9.8.7"}

==> Release verification passed
tag: v9.8.7
LOG
cat <<'JSON'
{
  "status": "passed",
  "tag": "v9.8.7",
  "version": "9.8.7",
  "repo": "sleticalboy/CodeInsight-mcp",
  "gates": {
    "github_release": "passed",
    "github_asset_downloads": "metadata_only",
    "release_notes": "passed",
    "install_script": "passed",
    "installed_quickstart": "skipped",
    "docker": "skipped",
    "homebrew_remote_formula": "passed",
    "homebrew_fetch": "skipped"
  },
  "expected_assets": ["codeinsight-x86_64-unknown-linux-gnu.tar.gz"],
  "docker": {"image": "ghcr.io/sleticalboy/codeinsight-mcp", "skipped": true},
  "homebrew": {"tap": "sleticalboy/tap", "repo": "sleticalboy/homebrew-tap", "skipped": true},
  "installed_quickstart": {
    "binary": "-",
    "skipped": true,
    "coverage": ["version", "index", "overview", "context-pack", "agent-route", "mcp_stdio", "mcp_agent_route", "agent_route_execution_plan", "reading_plan_focus", "reading_plan_question", "reading_plan_reason", "selection_reason", "selection_rank", "continuation_evidence"]
  }
}
JSON
EOF
  chmod +x "$fake_verify"

  cat >"$fake_update" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

test "$1" = "--evidence-json-file"
test "$2" = "$CODEINSIGHT_EXPECTED_EVIDENCE_JSON_FILE"
test -s "$3"
test "$4" = "$CODEINSIGHT_EXPECTED_STATUS_DOC"
cp "$3" "$4"
EOF
  chmod +x "$fake_update"

  cat >"$fake_handoff" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

generate_evidence=0
evidence_json_file=""
verification_json_file=""
json_output_file=""
output_file=""
evidence_branch=""
evidence_head_sha=""
evidence_run_id=""
repo=""
evidence_summary_script=""
tag=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --generate-evidence)
      generate_evidence=1
      ;;
    --evidence-json)
      shift
      evidence_json_file="$1"
      ;;
    --verification-json)
      shift
      verification_json_file="$1"
      ;;
    --json-output)
      shift
      json_output_file="$1"
      ;;
    --output)
      shift
      output_file="$1"
      ;;
    --evidence-branch)
      shift
      evidence_branch="$1"
      ;;
    --evidence-head-sha)
      shift
      evidence_head_sha="$1"
      ;;
    --evidence-run-id)
      shift
      evidence_run_id="$1"
      ;;
    --repo)
      shift
      repo="$1"
      ;;
    --release-evidence-summary-script)
      shift
      evidence_summary_script="$1"
      ;;
    *)
      tag="$1"
      ;;
  esac
  shift
done

test "$tag" = "v9.8.7"
test "$evidence_json_file" = "$CODEINSIGHT_EXPECTED_EVIDENCE_JSON_FILE"
test -s "$verification_json_file"
test "$json_output_file" = "$CODEINSIGHT_EXPECTED_HANDOFF_JSON_FILE"
test "$output_file" = "$CODEINSIGHT_EXPECTED_HANDOFF_FILE"

if [ "${CODEINSIGHT_EXPECT_GENERATED_EVIDENCE:-0}" = "1" ]; then
  test "$generate_evidence" = "1"
  test "$evidence_branch" = "release-smoke"
  test "$evidence_head_sha" = "abc123"
  test "$evidence_run_id" = "987654"
  test "$repo" = "example/codeinsight"
  test "$evidence_summary_script" = "$CODEINSIGHT_EXPECTED_RELEASE_EVIDENCE_SUMMARY_SCRIPT"
  mkdir -p "$(dirname "$evidence_json_file")"
  printf '{"schema_version":1,"tag":"v9.8.7","generated":true}\n' >"$evidence_json_file"
else
  test "$generate_evidence" = "0"
fi

mkdir -p "$(dirname "$json_output_file")" "$(dirname "$output_file")"
printf '{"schema_version":1,"tag":"v9.8.7"}\n' >"$json_output_file"
printf '## v9.8.7 release handoff\n' >"$output_file"
echo "release handoff summary written: $output_file"
echo "release handoff JSON written: $json_output_file"
EOF
  chmod +x "$fake_handoff"

  cat >"$fake_release_evidence_summary" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

echo "fake release evidence summary should be handled by fake handoff" >&2
exit 1
EOF
  chmod +x "$fake_release_evidence_summary"

  mkdir -p "$(dirname "$evidence_json_file")"
  echo '{"schema_version":1}' >"$evidence_json_file"

  CODEINSIGHT_EXPECTED_STATUS_DOC="$status_doc" \
    CODEINSIGHT_EXPECTED_EVIDENCE_JSON_FILE="$evidence_json_file" \
    CODEINSIGHT_EXPECTED_HANDOFF_FILE="$handoff_file" \
    CODEINSIGHT_EXPECTED_HANDOFF_JSON_FILE="$handoff_json_file" \
    CODEINSIGHT_VERIFY_RELEASE_SCRIPT="$fake_verify" \
    CODEINSIGHT_UPDATE_RELEASE_STATUS_SCRIPT="$fake_update" \
    CODEINSIGHT_RELEASE_HANDOFF_SUMMARY_SCRIPT="$fake_handoff" \
    CODEINSIGHT_RELEASE_HANDOFF_DIR="$TEMP_DIR/release-handoff" \
    "$ROOT_DIR/scripts/post-release-verify.sh" \
    --summary-file "$summary_file" \
    --status-doc "$status_doc" \
    --evidence-json-file "$evidence_json_file" \
    --handoff \
    --skip-docker \
    --skip-homebrew \
    --skip-installed-quickstart \
    --allow-asset-download-unreachable \
    9.8.7 >"$TEMP_DIR/post.out"

  jq -e '
    .tag == "v9.8.7" and
    .gates.github_asset_downloads == "metadata_only" and
    .gates.docker == "skipped"
  ' "$summary_file" >/dev/null
  jq -e '.schema_version == 1 and .tag == "v9.8.7"' "$handoff_json_file" >/dev/null
  cmp "$summary_file" "$status_doc"
  grep -q "## v9.8.7 release handoff" "$handoff_file"
  grep -q "release handoff summary written: $handoff_file" "$TEMP_DIR/post.out"
  grep -q "release handoff JSON written: $handoff_json_file" "$TEMP_DIR/post.out"
  grep -q "post-release verification passed" "$TEMP_DIR/post.out"
  grep -q "summary: $summary_file" "$TEMP_DIR/post.out"
  grep -q "status: $status_doc" "$TEMP_DIR/post.out"
  grep -q "evidence_json: $evidence_json_file" "$TEMP_DIR/post.out"
  grep -q "handoff_json: $handoff_json_file" "$TEMP_DIR/post.out"
  grep -q "handoff: $handoff_file" "$TEMP_DIR/post.out"

  CODEINSIGHT_EXPECTED_STATUS_DOC="$generated_status_doc" \
    CODEINSIGHT_EXPECTED_EVIDENCE_JSON_FILE="$generated_evidence_json_file" \
    CODEINSIGHT_EXPECTED_HANDOFF_FILE="$generated_handoff_file" \
    CODEINSIGHT_EXPECTED_HANDOFF_JSON_FILE="$generated_handoff_json_file" \
    CODEINSIGHT_EXPECT_GENERATED_EVIDENCE=1 \
    CODEINSIGHT_EXPECTED_RELEASE_EVIDENCE_SUMMARY_SCRIPT="$fake_release_evidence_summary" \
    CODEINSIGHT_VERIFY_RELEASE_SCRIPT="$fake_verify" \
    CODEINSIGHT_UPDATE_RELEASE_STATUS_SCRIPT="$fake_update" \
    CODEINSIGHT_RELEASE_HANDOFF_SUMMARY_SCRIPT="$fake_handoff" \
    CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT="$fake_release_evidence_summary" \
    "$ROOT_DIR/scripts/post-release-verify.sh" \
    --summary-file "$generated_summary_file" \
    --status-doc "$generated_status_doc" \
    --evidence-json-file "$generated_evidence_json_file" \
    --handoff-output "$generated_handoff_file" \
    --handoff-json-output "$generated_handoff_json_file" \
    --generate-evidence-for-handoff \
    --evidence-branch release-smoke \
    --evidence-head-sha abc123 \
    --evidence-run-id 987654 \
    --repo example/codeinsight \
    --skip-docker \
    --skip-homebrew \
    --skip-installed-quickstart \
    --allow-asset-download-unreachable \
    9.8.7 >"$TEMP_DIR/post-generated.out"

  jq -e '.schema_version == 1 and .tag == "v9.8.7" and .generated == true' "$generated_evidence_json_file" >/dev/null
  jq -e '.schema_version == 1 and .tag == "v9.8.7"' "$generated_handoff_json_file" >/dev/null
  cmp "$generated_summary_file" "$generated_status_doc"
  grep -q "## v9.8.7 release handoff" "$generated_handoff_file"
  grep -q "post-release verification passed" "$TEMP_DIR/post-generated.out"
  grep -q "evidence_json: $generated_evidence_json_file" "$TEMP_DIR/post-generated.out"
  grep -q "handoff_json: $generated_handoff_json_file" "$TEMP_DIR/post-generated.out"
  grep -q "handoff: $generated_handoff_file" "$TEMP_DIR/post-generated.out"

  echo "post-release verify smoke passed"
}

main "$@"
