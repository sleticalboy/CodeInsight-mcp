# Embedding Providers

CodeInsight keeps semantic search optional. The default path is still local
indexing plus deterministic syntax, symbol, reference, dependency, and call
graph signals.

## Current Support

`semantic-index` always creates local source-text chunks from the existing
SQLite project index.

Embeddings are generated only when an embedding provider is configured:

```bash
CODEINSIGHT_EMBEDDING_PROVIDER=local-hash codeinsight semantic-index /path/to/repo
CODEINSIGHT_EMBEDDING_PROVIDER=local-hash codeinsight semantic-search /path/to/repo "auth flow"

CODEINSIGHT_EMBEDDING_PROVIDER=ollama codeinsight semantic-index /path/to/repo
CODEINSIGHT_EMBEDDING_PROVIDER=ollama codeinsight semantic-search /path/to/repo "auth flow"

CODEINSIGHT_EMBEDDING_PROVIDER=openai CODEINSIGHT_OPENAI_API_KEY=... codeinsight semantic-index /path/to/repo
CODEINSIGHT_EMBEDDING_PROVIDER=openai CODEINSIGHT_OPENAI_API_KEY=... codeinsight semantic-search /path/to/repo "auth flow"
```

Supported provider values:

| Value | Status | Model | Network | Purpose |
| --- | --- | --- | --- | --- |
| `local-hash` | Implemented | `local-hash-v1` | No | Deterministic preview vectors for smoke tests and local-only demos |
| `local` | Alias | `local-hash-v1` | No | Short alias for `local-hash` |
| `ollama` | Implemented preview | `embeddinggemma` by default | Local HTTP | Local Ollama embeddings through `/api/embed` |
| `openai` | Implemented preview | `text-embedding-3-small` by default | HTTPS | OpenAI-compatible embeddings through `/embeddings` |
| `disabled` | Implemented | `disabled` | No | Explicitly disable embedding generation |
| `none` | Alias | `disabled` | No | Short alias for `disabled` |

If `CODEINSIGHT_EMBEDDING_PROVIDER` is unset, `semantic-index` stores chunks
without vectors and `semantic-search` fails with a configuration error.

Check the current provider configuration without making a network call:

```bash
codeinsight embedding-status
codeinsight embedding-status /path/to/repo
```

The optional repository argument adds local semantic chunk counts and the
number of vectors stored for the currently selected provider/model. MCP
clients can use the equivalent `embedding_status` tool before deciding whether
to run `semantic_index` or `semantic_search`.

`context_pack` also checks the selected provider/model. When vectors exist for
that pair, it embeds the task text and adds top vector matches as context
candidates. If no provider is configured, no matching vectors exist, or the
optional provider call fails, `context_pack` keeps using deterministic lexical,
symbol, reference, dependency, call graph, and semantic chunk fallback signals.
The returned `semantic_status` object tells clients whether vector candidates,
fallback candidates, selected vector ranges, or selected fallback ranges were
present, and includes a recommendation such as running `semantic-index` for the
selected provider/model.

## Local-Hash Boundary

`local-hash` is intentionally simple:

- It uses deterministic token hashing.
- It does not call external services.
- It is stable enough for tests and smoke checks.
- It is not a production semantic-quality embedding model.

Use it to validate indexing, storage, query flow, MCP wiring, and release
smoke tests. Do not use it to evaluate semantic relevance quality.

## Ollama Boundary

`ollama` is the first local HTTP embedding provider. It calls the Ollama
`/api/embed` endpoint documented in the
[Ollama API reference](https://docs.ollama.com/api) and remains opt-in.

Defaults:

- `CODEINSIGHT_OLLAMA_BASE_URL=http://127.0.0.1:11434`
- `CODEINSIGHT_OLLAMA_EMBEDDING_MODEL=embeddinggemma`
- `CODEINSIGHT_OLLAMA_TIMEOUT_SECS=30`
- `CODEINSIGHT_EMBEDDING_BATCH_SIZE=64`

Example:

```bash
ollama pull embeddinggemma
CODEINSIGHT_EMBEDDING_PROVIDER=ollama codeinsight semantic-index /path/to/repo
CODEINSIGHT_EMBEDDING_PROVIDER=ollama codeinsight semantic-search /path/to/repo "request validation"
```

Use another local embedding model:

```bash
CODEINSIGHT_EMBEDDING_PROVIDER=ollama \
CODEINSIGHT_OLLAMA_EMBEDDING_MODEL=nomic-embed-text \
codeinsight semantic-index /path/to/repo
```

Optional smoke test:

```bash
scripts/ollama-semantic-smoke.sh
```

The smoke test skips clearly when Ollama is unreachable or the selected model
is missing.

Current limitations:

- Only `http://` Ollama base URLs are supported.
- The provider uses a small built-in HTTP client to avoid adding an external
  dependency.
- It expects Ollama to return one embedding per input text.
- It does not pull models automatically.

## OpenAI-Compatible Boundary

`openai` is the first external HTTPS embedding provider. It calls an
OpenAI-compatible `/embeddings` endpoint with `model`, `input`, and
`encoding_format: "float"`, then orders returned vectors by the response
`data[].index` field.

Required:

- `CODEINSIGHT_OPENAI_API_KEY`

Defaults:

- `CODEINSIGHT_OPENAI_BASE_URL=https://api.openai.com/v1`
- `CODEINSIGHT_OPENAI_EMBEDDING_MODEL=text-embedding-3-small`
- `CODEINSIGHT_OPENAI_TIMEOUT_SECS=30`
- `CODEINSIGHT_EMBEDDING_BATCH_SIZE=64`

Example status check:

```bash
CODEINSIGHT_EMBEDDING_PROVIDER=openai \
CODEINSIGHT_OPENAI_API_KEY=... \
codeinsight embedding-status
```

`embedding-status` reports whether the API key is configured, but never prints
the key value.

Example indexing and search:

```bash
CODEINSIGHT_EMBEDDING_PROVIDER=openai \
CODEINSIGHT_OPENAI_API_KEY=... \
codeinsight semantic-index /path/to/repo

CODEINSIGHT_EMBEDDING_PROVIDER=openai \
CODEINSIGHT_OPENAI_API_KEY=... \
codeinsight semantic-search /path/to/repo "request validation"
```

Current limitations:

- `semantic-index` batches embedding requests with
  `CODEINSIGHT_EMBEDDING_BATCH_SIZE`, defaulting to 64 chunks per request.
- It does not retry rate limits or transient HTTP failures yet.
- It stores vectors under `(provider, model)` like all other providers.

## External Provider Contract

Additional external providers should follow this contract:

- No provider is enabled by default.
- Provider selection remains explicit through `CODEINSIGHT_EMBEDDING_PROVIDER`.
- Provider-specific secrets must come from environment variables, never CLI
  arguments or MCP payloads.
- `semantic-index` must write vectors under `(provider, model)` so multiple
  providers can coexist in SQLite.
- `semantic-index` must batch external provider requests with
  `CODEINSIGHT_EMBEDDING_BATCH_SIZE` so large repositories do not require one
  huge provider request.
- `semantic-search` must query only vectors matching the configured provider
  and model.
- Provider errors must be explicit: missing credentials, failed network calls,
  invalid response shape, dimension mismatch, and rate limits should be
  distinguishable.
- Provider status reporting must not make network requests.
- A provider must return exactly one vector per input text chunk.
- Vector dimensions must be stored with each embedding row.
- Network providers must preserve the local-first default: no outbound request
  unless explicitly configured.

Planned provider names and environment boundaries:

| Provider | Planned value | Required env | Optional env |
| --- | --- | --- | --- |
| Qdrant-backed retrieval | `qdrant` | `CODEINSIGHT_QDRANT_URL` | `CODEINSIGHT_QDRANT_API_KEY`, `CODEINSIGHT_QDRANT_COLLECTION` |

Qdrant is a retrieval backend, not an embedding model by itself. A Qdrant path
still needs a vector-generation provider or a documented convention for
precomputed vectors.

## Error Semantics

Expected errors:

- Unset provider:
  `embedding provider is not configured; set CODEINSIGHT_EMBEDDING_PROVIDER=local-hash ...`
- Unknown provider:
  `unsupported embedding provider '...'; supported providers: ...`
- Missing vector index:
  `semantic search index is empty for provider ... model ...; run semantic-index ...`
- Bad provider response:
  `embedding provider returned N vectors for M chunks in batch B`
- Ollama unavailable:
  `ollama embedding provider is unreachable at ...`
- OpenAI unavailable or rejected:
  `openai embedding provider returned HTTP ...`

These messages are part of the CLI/MCP contract and should stay stable enough
for AI agents to recover by running the next command.

## Minimal Implementation Path

The next production-grade provider should be added in this order:

1. Add a provider config parser with clear env validation.
2. Add a provider implementation behind the existing `EmbeddingProvider` trait.
3. Add unit tests for config errors and response-shape errors.
4. Add a smoke script that can run only when credentials or a local endpoint are
   present, and skips clearly otherwise.
5. Document the provider in this file and in the known limitations.
