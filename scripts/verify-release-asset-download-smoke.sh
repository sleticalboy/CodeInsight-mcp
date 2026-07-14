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

for arg in "$@"; do
  if [ "$arg" = "--head" ]; then
    echo "simulated HEAD failure" >&2
    exit 22
  fi
done

exit 0
EOF
  chmod +x "$SMOKE_TEMP_DIR/fake-bin/curl"

  export CODEINSIGHT_CURL_LOG="$SMOKE_TEMP_DIR/curl.log"
  export PATH="$SMOKE_TEMP_DIR/fake-bin:$PATH"
  export CODEINSIGHT_VERIFY_RELEASE_NO_MAIN=1

  # shellcheck disable=SC1091
  source "$ROOT_DIR/scripts/verify-release.sh"

  verify_release_asset_download_url v9.8.7 codeinsight-x86_64-unknown-linux-gnu.tar.gz >"$SMOKE_TEMP_DIR/out" 2>"$SMOKE_TEMP_DIR/err"

  grep -qx -- '--head' "$CODEINSIGHT_CURL_LOG"
  grep -qx -- '--range' "$CODEINSIGHT_CURL_LOG"
  grep -qx -- '0-0' "$CODEINSIGHT_CURL_LOG"
  grep -q 'asset HEAD check failed, retrying with ranged GET' "$SMOKE_TEMP_DIR/err"

  echo "verify-release asset download smoke passed"
}

main "$@"
