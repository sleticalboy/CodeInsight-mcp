#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-$ROOT_DIR/target/release/codeinsight}"
TEMP_DIR=""

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

build_binary_if_needed() {
  if [ ! -x "$CODEINSIGHT_BIN" ]; then
    cargo build --locked --release --manifest-path "$ROOT_DIR/Cargo.toml"
  fi
}

create_fixture() {
  TEMP_DIR="$(mktemp -d)"
  mkdir -p "$TEMP_DIR/repo/src"

  cat >"$TEMP_DIR/repo/src/auth.py" <<'EOF'
class AuthService:
    def login(self, session):
        return validate_session_cookie(session)

def validate_session_cookie(session):
    return session.get("cookie") == "fresh"
EOF

  cat >"$TEMP_DIR/repo/src/auth_notes.py" <<'EOF'
def session_cookie_notes():
    return "session cookie behavior depends on a fresh cookie value"
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
  local chunks_json="$TEMP_DIR/chunks.json"
  local embeddings_json="$TEMP_DIR/embeddings.json"
  local reused_embeddings_json="$TEMP_DIR/reused-embeddings.json"
  local search_json="$TEMP_DIR/search.json"
  local error_log="$TEMP_DIR/search-error.log"

  "$CODEINSIGHT_BIN" index "$repo_dir" --force >"$index_json"
  jq -e '.indexed_files == 2 and (.errors | length) == 0' "$index_json" >/dev/null

  if "$CODEINSIGHT_BIN" semantic-search "$repo_dir" "session cookie behavior" >"$search_json" 2>"$error_log"; then
    echo "semantic-search should fail without CODEINSIGHT_EMBEDDING_PROVIDER" >&2
    exit 1
  fi
  grep -q "embedding provider is not configured" "$error_log"

  "$CODEINSIGHT_BIN" semantic-index "$repo_dir" --chunk-lines 10 >"$chunks_json"
  jq -e '.vector_status == "chunks_indexed_without_embeddings" and .chunks > 0 and .embeddings == 0' "$chunks_json" >/dev/null

  CODEINSIGHT_EMBEDDING_PROVIDER=local-hash \
    "$CODEINSIGHT_BIN" semantic-index "$repo_dir" --chunk-lines 10 >"$embeddings_json"
  jq -e '.provider == "local-hash" and .vector_status == "embeddings_indexed" and .embeddings == .chunks and .embeddings > 0 and .embeddings_generated == .chunks and .embeddings_reused == 0' "$embeddings_json" >/dev/null

  CODEINSIGHT_EMBEDDING_PROVIDER=local-hash \
    "$CODEINSIGHT_BIN" semantic-index "$repo_dir" --chunk-lines 10 >"$reused_embeddings_json"
  jq -e '.provider == "local-hash" and .vector_status == "embeddings_indexed" and .embeddings == .chunks and .embeddings_generated == 0 and .embeddings_reused == .chunks' "$reused_embeddings_json" >/dev/null

  CODEINSIGHT_EMBEDDING_PROVIDER=local-hash \
    "$CODEINSIGHT_BIN" semantic-search "$repo_dir" "session cookie behavior" --limit 5 >"$search_json"
  jq -e '
    length > 0
    and any(.[]; .file == "src/auth_notes.py" and (.excerpt | contains("session cookie behavior")) and .score > 0)
  ' "$search_json" >/dev/null

  echo "Semantic smoke passed"
  echo "root: $repo_dir"
  echo "indexed_files: $(jq -r '.indexed_files' "$index_json")"
  echo "chunks: $(jq -r '.chunks' "$embeddings_json")"
  echo "embeddings: $(jq -r '.embeddings' "$embeddings_json")"
  echo "top_result: $(jq -r '.[0].file' "$search_json")"
}

main "$@"
