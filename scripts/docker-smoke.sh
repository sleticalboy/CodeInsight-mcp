#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${CODEINSIGHT_DOCKER_IMAGE:-codeinsight:local}"
DOCKER_PLATFORM="${CODEINSIGHT_DOCKER_PLATFORM:-}"
TEMP_DIR=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

main() {
  require_command docker
  require_command python3

  if [ -n "$DOCKER_PLATFORM" ]; then
    docker build --platform "$DOCKER_PLATFORM" -t "$IMAGE" "$ROOT_DIR"
    docker run --rm --platform "$DOCKER_PLATFORM" "$IMAGE" --help >/dev/null
  else
    docker build -t "$IMAGE" "$ROOT_DIR"
    docker run --rm "$IMAGE" --help >/dev/null
  fi

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  cat >"$TEMP_DIR/app.py" <<'PY'
class SmokeService:
    def run(self):
        return "ok"
PY

  if [ -n "$DOCKER_PLATFORM" ]; then
    docker run --rm \
      --platform "$DOCKER_PLATFORM" \
      -v "$TEMP_DIR:/workspace" \
      "$IMAGE" index /workspace --force \
      | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["indexed_files"] == 1, data; assert not data["errors"], data'
  else
    docker run --rm \
      -v "$TEMP_DIR:/workspace" \
      "$IMAGE" index /workspace --force \
      | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["indexed_files"] == 1, data; assert not data["errors"], data'
  fi

  echo "docker smoke passed"
  echo "image: $IMAGE"
  if [ -n "$DOCKER_PLATFORM" ]; then
    echo "platform: $DOCKER_PLATFORM"
  fi
}

main "$@"
