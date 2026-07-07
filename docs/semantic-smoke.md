# Semantic Smoke Test

`scripts/semantic-smoke.sh` verifies the local semantic search loop without any
external embedding service.

It creates a temporary Python fixture and checks:

- `index` builds a clean project index.
- `semantic-search` fails clearly when `CODEINSIGHT_EMBEDDING_PROVIDER` is not configured.
- `semantic-index` stores source-text chunks without embeddings by default.
- `CODEINSIGHT_EMBEDDING_PROVIDER=local-hash semantic-index` generates deterministic local embeddings.
- `CODEINSIGHT_EMBEDDING_PROVIDER=local-hash semantic-search` returns a relevant chunk with a positive score.

Run it from the repository root:

```bash
scripts/semantic-smoke.sh
```

Use an existing binary:

```bash
CODEINSIGHT_BIN=target/release/codeinsight scripts/semantic-smoke.sh
```

The smoke test is deterministic and local-only. It is meant as a release gate
for the preview `semantic-index` and `semantic-search` path, not as a quality
benchmark for production embeddings.

For an optional local Ollama embedding smoke test, run:

```bash
scripts/ollama-semantic-smoke.sh
```

That script uses `CODEINSIGHT_EMBEDDING_PROVIDER=ollama` and skips when Ollama
is unreachable or the selected model is missing.

For an optional OpenAI-compatible embedding smoke test, run:

```bash
CODEINSIGHT_OPENAI_API_KEY=... scripts/openai-semantic-smoke.sh
```

That script uses `CODEINSIGHT_EMBEDDING_PROVIDER=openai`, defaults to
`CODEINSIGHT_EMBEDDING_BATCH_SIZE=1` to exercise batched indexing, checks that
`embedding-status` does not print the API key, and skips clearly when
`CODEINSIGHT_OPENAI_API_KEY` is not configured.
