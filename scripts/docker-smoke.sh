#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${CODEINSIGHT_DOCKER_IMAGE:-codeinsight:local}"
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

  docker build -t "$IMAGE" "$ROOT_DIR"
  docker run --rm "$IMAGE" --help >/dev/null

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  cat >"$TEMP_DIR/app.py" <<'PY'
class SmokeService:
    def run(self):
        return "ok"
PY

  docker run --rm \
    -v "$TEMP_DIR:/workspace" \
    "$IMAGE" index /workspace --force \
    | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["indexed_files"] == 1, data; assert not data["errors"], data'

  echo "docker smoke passed"
  echo "image: $IMAGE"
}

main "$@"
