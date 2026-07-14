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

  local target="x86_64-unknown-linux-gnu"
  local asset="$TEMP_DIR/codeinsight-$target.tar.gz"
  local package_dir="$TEMP_DIR/package/codeinsight-$target"
  local fake_bin="$TEMP_DIR/fake-bin"
  local install_dir="$TEMP_DIR/install"

  mkdir -p "$package_dir" "$fake_bin"
  cat >"$package_dir/codeinsight" <<'EOF'
#!/usr/bin/env sh
exit 0
EOF
  chmod +x "$package_dir/codeinsight"
  tar -C "$TEMP_DIR/package" -czf "$asset" "codeinsight-$target"

  cat >"$fake_bin/gh" <<'EOF'
#!/usr/bin/env sh
if [ "${CODEINSIGHT_FAKE_GH_SLEEP:-}" = "1" ]; then
  sleep 30
  exit 0
fi
exit 42
EOF
  chmod +x "$fake_bin/gh"

  cat >"$fake_bin/curl" <<'EOF'
#!/usr/bin/env sh
set -eu

output=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o)
      output="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [ -z "$output" ]; then
  echo "fake curl did not receive -o" >&2
  exit 1
fi

cp "$CODEINSIGHT_FAKE_ASSET" "$output"
EOF
  chmod +x "$fake_bin/curl"

  PATH="$fake_bin:$PATH" \
    CODEINSIGHT_TARGET="$target" \
    CODEINSIGHT_VERSION=v9.8.7 \
    CODEINSIGHT_FAKE_ASSET="$asset" \
    INSTALL_DIR="$install_dir" \
    sh "$ROOT_DIR/scripts/install.sh" >"$TEMP_DIR/install.out" 2>"$TEMP_DIR/install.err"

  test -x "$install_dir/codeinsight"
  grep -q 'gh release download failed; falling back to curl' "$TEMP_DIR/install.err"

  rm -rf "$install_dir"
  PATH="$fake_bin:$PATH" \
    CODEINSIGHT_TARGET="$target" \
    CODEINSIGHT_VERSION=v9.8.7 \
    CODEINSIGHT_FAKE_ASSET="$asset" \
    CODEINSIGHT_FAKE_GH_SLEEP=1 \
    CODEINSIGHT_DOWNLOAD_TIMEOUT_SECONDS=1 \
    INSTALL_DIR="$install_dir" \
    sh "$ROOT_DIR/scripts/install.sh" >"$TEMP_DIR/install-timeout.out" 2>"$TEMP_DIR/install-timeout.err"

  test -x "$install_dir/codeinsight"
  grep -q 'download command timed out after 1s' "$TEMP_DIR/install-timeout.err"
  grep -q 'gh release download failed; falling back to curl' "$TEMP_DIR/install-timeout.err"

  echo "install fallback smoke passed"
}

main "$@"
