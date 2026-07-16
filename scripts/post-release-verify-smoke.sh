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
  local evidence_file="$TEMP_DIR/release-evidence/v9.8.7.md"
  local fake_verify="$TEMP_DIR/verify-release.sh"
  local fake_update="$TEMP_DIR/update-release-status.sh"

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
    "coverage": ["version", "index", "overview", "context-pack", "agent-route", "mcp_stdio", "mcp_agent_route"]
  }
}
JSON
EOF
  chmod +x "$fake_verify"

  cat >"$fake_update" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

test "$1" = "--evidence-file"
test "$2" = "$CODEINSIGHT_EXPECTED_EVIDENCE_FILE"
test -s "$3"
test "$4" = "$CODEINSIGHT_EXPECTED_STATUS_DOC"
cp "$3" "$4"
EOF
  chmod +x "$fake_update"

  mkdir -p "$(dirname "$evidence_file")"
  echo "release evidence summary" >"$evidence_file"

  CODEINSIGHT_EXPECTED_STATUS_DOC="$status_doc" \
    CODEINSIGHT_EXPECTED_EVIDENCE_FILE="$evidence_file" \
    CODEINSIGHT_VERIFY_RELEASE_SCRIPT="$fake_verify" \
    CODEINSIGHT_UPDATE_RELEASE_STATUS_SCRIPT="$fake_update" \
    "$ROOT_DIR/scripts/post-release-verify.sh" \
    --summary-file "$summary_file" \
    --status-doc "$status_doc" \
    --evidence-file "$evidence_file" \
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
  cmp "$summary_file" "$status_doc"
  grep -q "post-release verification passed" "$TEMP_DIR/post.out"
  grep -q "summary: $summary_file" "$TEMP_DIR/post.out"
  grep -q "status: $status_doc" "$TEMP_DIR/post.out"
  grep -q "evidence: $evidence_file" "$TEMP_DIR/post.out"

  echo "post-release verify smoke passed"
}

main "$@"
