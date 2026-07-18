#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

main() {
  cd "$ROOT_DIR"

  local script
  for script in scripts/*.sh; do
    bash -n "$script"
  done

  echo "script syntax smoke passed"
}

main "$@"
