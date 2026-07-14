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
  cat >"$SMOKE_TEMP_DIR/fake-bin/docker" <<'EOF'
#!/usr/bin/env sh
echo 'Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?' >&2
exit 1
EOF
  chmod +x "$SMOKE_TEMP_DIR/fake-bin/docker"

  if PATH="$SMOKE_TEMP_DIR/fake-bin:$PATH" \
    CODEINSIGHT_VERIFY_RELEASE_NO_MAIN=1 \
    bash -c 'source "$1"; verify_docker_environment' bash "$ROOT_DIR/scripts/verify-release.sh" \
    >"$SMOKE_TEMP_DIR/out" 2>"$SMOKE_TEMP_DIR/err"; then
    echo "verify_docker_environment unexpectedly succeeded with failing docker" >&2
    exit 1
  fi

  grep -q 'Docker command failed while checking Docker daemon availability' "$SMOKE_TEMP_DIR/err"
  grep -q 'Docker is not usable in this shell' "$SMOKE_TEMP_DIR/err"
  grep -q 'CODEINSIGHT_SKIP_DOCKER=1' "$SMOKE_TEMP_DIR/err"

  echo "verify-release docker failure smoke passed"
}

main "$@"
