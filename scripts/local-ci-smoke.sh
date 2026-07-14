#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

main() {
  cd "$ROOT_DIR"

  cargo fmt --check
  cargo test --locked
  bash -n scripts/*.sh
  scripts/release-tooling-smoke.sh
  scripts/docs-smoke.sh
  git diff --check

  echo "local CI smoke passed"
}

main "$@"
