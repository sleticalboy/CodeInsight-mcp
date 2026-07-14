#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

main() {
  "$ROOT_DIR/scripts/docs-link-smoke.sh"
  "$ROOT_DIR/scripts/docs-positioning-smoke.sh"
  "$ROOT_DIR/scripts/docs-benchmark-smoke.sh"

  echo "docs smoke passed"
}

main "$@"
