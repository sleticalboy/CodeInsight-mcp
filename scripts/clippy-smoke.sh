#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

main() {
  cd "$ROOT_DIR"

  cargo clippy --locked --all-targets -- -D warnings -A clippy::too_many_arguments

  echo "clippy smoke passed"
}

main "$@"
