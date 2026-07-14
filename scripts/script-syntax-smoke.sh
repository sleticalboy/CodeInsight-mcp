#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

main() {
  cd "$ROOT_DIR"

  bash -n scripts/*.sh

  echo "script syntax smoke passed"
}

main "$@"
