#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-$ROOT_DIR/target/release/codeinsight}"
OPENAI_MODEL="${CODEINSIGHT_OPENAI_EMBEDDING_MODEL:-text-embedding-3-small}"
OPENAI_BASE_URL="${CODEINSIGHT_OPENAI_BASE_URL:-https://api.openai.com/v1}"
EMBEDDING_BATCH_SIZE="${CODEINSIGHT_EMBEDDING_BATCH_SIZE:-1}"
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

  cat >"$TEMP_DIR/repo/src/password.py" <<'EOF'
def issue_password_reset_token(user):
    return f"reset-token:{user['id']}"
EOF

  cat >"$TEMP_DIR/repo/src/password_notes.py" <<'EOF'
def reset_token_notes():
    return "password reset token rotation protects account recovery flows"
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

  if [ -z "${CODEINSIGHT_OPENAI_API_KEY:-}" ]; then
    echo "OpenAI semantic smoke skipped"
    echo "reason: CODEINSIGHT_OPENAI_API_KEY is not configured"
    exit 0
  fi

  build_binary_if_needed
  create_fixture
  trap cleanup EXIT INT TERM

  local repo_dir="$TEMP_DIR/repo"
  local index_json="$TEMP_DIR/index.json"
  local status_json="$TEMP_DIR/status.json"
  local embeddings_json="$TEMP_DIR/embeddings.json"
  local search_json="$TEMP_DIR/search.json"

  "$CODEINSIGHT_BIN" index "$repo_dir" --force >"$index_json"
  jq -e '.indexed_files == 2 and (.errors | length) == 0' "$index_json" >/dev/null

  CODEINSIGHT_EMBEDDING_PROVIDER=openai \
    "$CODEINSIGHT_BIN" embedding-status "$repo_dir" >"$status_json"
  jq -e '.provider == "openai" and .configured == true and .openai.api_key_configured == true' "$status_json" >/dev/null
  if grep -Fq -- "$CODEINSIGHT_OPENAI_API_KEY" "$status_json"; then
    echo "embedding-status leaked CODEINSIGHT_OPENAI_API_KEY" >&2
    exit 1
  fi

  CODEINSIGHT_EMBEDDING_PROVIDER=openai \
    CODEINSIGHT_EMBEDDING_BATCH_SIZE="$EMBEDDING_BATCH_SIZE" \
    "$CODEINSIGHT_BIN" semantic-index "$repo_dir" --chunk-lines 10 >"$embeddings_json"
  jq -e '.provider == "openai" and .vector_status == "embeddings_indexed" and .embeddings == .chunks and .embeddings > 0' "$embeddings_json" >/dev/null

  CODEINSIGHT_EMBEDDING_PROVIDER=openai \
    "$CODEINSIGHT_BIN" semantic-search "$repo_dir" "password reset token rotation" --limit 5 >"$search_json"
  jq -e '
    length > 0
    and all(.[]; .score > 0)
    and any(.[]; .file == "src/password_notes.py")
  ' "$search_json" >/dev/null

  echo "OpenAI semantic smoke passed"
  echo "root: $repo_dir"
  echo "base_url: $OPENAI_BASE_URL"
  echo "model: $OPENAI_MODEL"
  echo "batch_size: $EMBEDDING_BATCH_SIZE"
  echo "chunks: $(jq -r '.chunks' "$embeddings_json")"
  echo "embeddings: $(jq -r '.embeddings' "$embeddings_json")"
  echo "top_result: $(jq -r '.[0].file' "$search_json")"
}

main "$@"
