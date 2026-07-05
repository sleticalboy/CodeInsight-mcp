#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

detect_target() {
  local os arch os_target arch_target
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) os_target="apple-darwin" ;;
    Linux) os_target="unknown-linux-gnu" ;;
    *)
      echo "unsupported operating system: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64 | amd64) arch_target="x86_64" ;;
    arm64 | aarch64) arch_target="aarch64" ;;
    *)
      echo "unsupported CPU architecture: $arch" >&2
      exit 1
      ;;
  esac

  printf "%s-%s" "$arch_target" "$os_target"
}

package_current_target() {
  local target="$1"
  local asset="$2"
  local name="codeinsight-$target"

  cargo build --locked --release --target "$target" --manifest-path "$ROOT_DIR/Cargo.toml"
  rm -rf "$ROOT_DIR/dist/$name"
  mkdir -p "$ROOT_DIR/dist/$name"
  cp "$ROOT_DIR/target/$target/release/codeinsight" "$ROOT_DIR/dist/$name/"
  cp "$ROOT_DIR/README.md" "$ROOT_DIR"/LICENSE* "$ROOT_DIR/dist/$name/" 2>/dev/null || true
  tar -C "$ROOT_DIR/dist" -czf "$asset" "$name"
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

main() {
  require_command cargo
  require_command tar

  local native_target target asset install_dir skip_verify
  native_target="$(detect_target)"
  target="${CODEINSIGHT_TARGET:-$native_target}"
  asset="${CODEINSIGHT_SMOKE_ASSET:-$ROOT_DIR/dist/codeinsight-$target.tar.gz}"

  if [ ! -f "$asset" ]; then
    package_current_target "$target" "$asset"
  fi

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM
  install_dir="$TEMP_DIR/bin"

  skip_verify="${CODEINSIGHT_SKIP_VERIFY:-}"
  if [ "$target" != "$native_target" ]; then
    skip_verify="1"
  fi

  CODEINSIGHT_TARGET="$target" \
    CODEINSIGHT_ASSET_PATH="$asset" \
    CODEINSIGHT_SKIP_VERIFY="$skip_verify" \
    INSTALL_DIR="$install_dir" \
    sh "$ROOT_DIR/scripts/install.sh"

  test -x "$install_dir/codeinsight"

  if [ "$skip_verify" != "1" ]; then
    CODEINSIGHT_BIN="$install_dir/codeinsight" "$ROOT_DIR/scripts/mcp-stdio-smoke.sh"
  fi

  echo "release install smoke passed"
  echo "target: $target"
  echo "asset: $asset"
}

main "$@"
