#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TEMP_DIR=""

cleanup() {
  if [ -n "$SMOKE_TEMP_DIR" ]; then
    rm -rf "$SMOKE_TEMP_DIR"
  fi
}

main() {
  SMOKE_TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$SMOKE_TEMP_DIR/fake-bin"
  cat >"$SMOKE_TEMP_DIR/fake-bin/curl" <<'EOF'
#!/usr/bin/env sh
set -eu

printf '%s\n' "$@" >>"$CODEINSIGHT_CURL_LOG"
echo "curl: (28) Failed to connect to github.com port 443 after 20000 ms: Timeout was reached" >&2
exit 28
EOF
  chmod +x "$SMOKE_TEMP_DIR/fake-bin/curl"

  export CODEINSIGHT_CURL_LOG="$SMOKE_TEMP_DIR/curl.log"
  export PATH="$SMOKE_TEMP_DIR/fake-bin:$PATH"
  export CODEINSIGHT_VERIFY_RELEASE_NO_MAIN=1

  # shellcheck disable=SC1091
  source "$ROOT_DIR/scripts/verify-release.sh"

  if verify_release_asset_download_url v9.8.7 codeinsight-x86_64-unknown-linux-gnu.tar.gz >"$SMOKE_TEMP_DIR/default.out" 2>"$SMOKE_TEMP_DIR/default.err"; then
    echo "asset download unexpectedly succeeded without metadata-only override" >&2
    exit 1
  fi

  grep -q 'direct download URL is not reachable from this machine' "$SMOKE_TEMP_DIR/default.err"
  grep -q 'CODEINSIGHT_ALLOW_ASSET_DOWNLOAD_UNREACHABLE=1' "$SMOKE_TEMP_DIR/default.err"

  export CODEINSIGHT_ALLOW_ASSET_DOWNLOAD_UNREACHABLE=1
  verify_release_asset_download_url v9.8.7 codeinsight-x86_64-unknown-linux-gnu.tar.gz >"$SMOKE_TEMP_DIR/allowed.out" 2>"$SMOKE_TEMP_DIR/allowed.err"

  grep -q 'continuing with metadata-only asset verification' "$SMOKE_TEMP_DIR/allowed.err"
  test "$ASSET_DOWNLOADS_METADATA_ONLY" = true

  release_verification_summary_json v9.8.7 9.8.7 >"$SMOKE_TEMP_DIR/summary.json"
  jq -e '.gates.github_asset_downloads == "metadata_only"' "$SMOKE_TEMP_DIR/summary.json" >/dev/null

  echo "verify-release asset unreachable smoke passed"
}

main "$@"
