#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-$ROOT_DIR/target/release/codeinsight}"
OLLAMA_MODEL="${CODEINSIGHT_OLLAMA_EMBEDDING_MODEL:-embeddinggemma}"
TEMP_DIR=""

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

build_binary_if_needed() {
  if [ ! -x "$CODEINSIGHT_BIN" ] || find "$ROOT_DIR/src" "$ROOT_DIR/Cargo.toml" "$ROOT_DIR/Cargo.lock" -newer "$CODEINSIGHT_BIN" | grep -q .; then
    cargo build --locked --release --manifest-path "$ROOT_DIR/Cargo.toml"
  fi
}

create_fixture() {
  TEMP_DIR="$(mktemp -d)"
  mkdir -p "$TEMP_DIR/repo/src"

  cat >"$TEMP_DIR/repo/src/search.py" <<'EOF'
def calculate_invoice_total(items):
    return sum(item["price"] for item in items)
EOF

  cat >"$TEMP_DIR/repo/src/notes.py" <<'EOF'
def billing_notes():
    return "invoice total calculation combines item prices"
EOF
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

main() {
  require_command cargo
  require_command jq
  build_binary_if_needed
  create_fixture
  trap cleanup EXIT INT TERM

  local repo_dir="$TEMP_DIR/repo"
  local index_json="$TEMP_DIR/index.json"
  local embeddings_json="$TEMP_DIR/embeddings.json"
  local search_json="$TEMP_DIR/search.json"
  local error_log="$TEMP_DIR/ollama-error.log"

  "$CODEINSIGHT_BIN" index "$repo_dir" --force >"$index_json"
  jq -e '.indexed_files == 2 and (.errors | length) == 0' "$index_json" >/dev/null

  if ! CODEINSIGHT_EMBEDDING_PROVIDER=ollama \
    "$CODEINSIGHT_BIN" semantic-index "$repo_dir" --chunk-lines 10 >"$embeddings_json" 2>"$error_log"; then
    if grep -Eq "unreachable|model|404|not found|connection refused" "$error_log"; then
      echo "Ollama semantic smoke skipped"
      echo "model: $OLLAMA_MODEL"
      sed -n '1,4p' "$error_log"
      exit 0
    fi
    cat "$error_log" >&2
    exit 1
  fi

  jq -e '.provider == "ollama" and .vector_status == "embeddings_indexed" and .embeddings == .chunks and .embeddings > 0' "$embeddings_json" >/dev/null

  CODEINSIGHT_EMBEDDING_PROVIDER=ollama \
    "$CODEINSIGHT_BIN" semantic-search "$repo_dir" "invoice total calculation" --limit 5 >"$search_json"
  jq -e 'length > 0 and all(.[]; .score > 0)' "$search_json" >/dev/null

  echo "Ollama semantic smoke passed"
  echo "root: $repo_dir"
  echo "model: $OLLAMA_MODEL"
  echo "chunks: $(jq -r '.chunks' "$embeddings_json")"
  echo "embeddings: $(jq -r '.embeddings' "$embeddings_json")"
  echo "top_result: $(jq -r '.[0].file' "$search_json")"
}

main "$@"
