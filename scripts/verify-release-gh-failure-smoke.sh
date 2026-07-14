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

  mkdir -p "$TEMP_DIR/fake-bin"
  cat >"$TEMP_DIR/fake-bin/gh" <<'EOF'
#!/usr/bin/env sh
echo 'non-200 OK status code: 401 Unauthorized body: {"message":"Requires authentication"}' >&2
exit 1
EOF
  chmod +x "$TEMP_DIR/fake-bin/gh"

  if PATH="$TEMP_DIR/fake-bin:$PATH" \
    CODEINSIGHT_SKIP_DOCKER=1 \
    CODEINSIGHT_SKIP_HOMEBREW=1 \
    "$ROOT_DIR/scripts/verify-release.sh" v9.8.7 >"$TEMP_DIR/out" 2>"$TEMP_DIR/err"; then
    echo "verify-release unexpectedly succeeded with failing gh" >&2
    exit 1
  fi

  grep -q 'GitHub CLI command failed while reading GitHub Release v9.8.7' "$TEMP_DIR/err"
  grep -q 'GitHub CLI authentication is not usable' "$TEMP_DIR/err"
  grep -q 'gh auth status' "$TEMP_DIR/err"

  echo "verify-release gh failure smoke passed"
}

main "$@"
