# CodeInsight MCP Server

CodeInsight MCP Server is a local-first code intelligence layer for AI coding agents.

The MVP focuses on a narrow, verifiable loop:

- index a local repository
- extract source symbols with Tree-sitter
- search symbols from a local SQLite index
- expose an MCP stdio server scaffold
- build toward agent-ready context packs

The product direction and execution plan live in:

- [Product prototype](docs/product-prototype.md)
- [Implementation plan](docs/implementation-plan.md)
- [MVP backlog](docs/mvp-backlog.md)
- [Known limitations](docs/known-limitations.md)
- [MCP client configuration](docs/mcp-client-config.md)
- [MCP client smoke test](docs/mcp-client-smoke.md)
- [Impact analysis](docs/impact-analysis.md)
- [Embedding providers](docs/embedding-providers.md)
- [Semantic smoke test](docs/semantic-smoke.md)
- [Smoke benchmark](docs/benchmark-v0.1.md)
- [Large repository benchmark](docs/benchmark-large.md)
- [Release runbook](docs/release-runbook.md)
- [Changelog](CHANGELOG.md)

## Current Status

This repository is an early MVP scaffold. It is not yet a complete MCP code-analysis server.

Implemented:

- Rust CLI entrypoint
- local SQLite index cache under `.codeinsight/`
- incremental indexing with file-hash skips and stale file cleanup
- index metadata with schema and index version tracking
- per-file indexing errors in reports without aborting the whole project scan
- Tree-sitter parsing for TypeScript/JavaScript, Python, Go, Rust, Java, C, C++, C#, PHP, and Ruby
- symbol extraction for common declarations
- repository overview, dependency graph, text reference search, impact analysis, context packs, and call graph tools with imported target hints
- relative file resolution for local dependency graph edges
- embedding provider interface, provider status reporting, and local semantic search paths over local vectors
- local semantic chunk index storage with optional deterministic local-hash embedding generation
- `index`, `init-config`, `config-status`, `overview`, `symbols`, `outline`, `dependency-graph`, `impact-analysis`, `find-references`, `semantic-search`, `semantic-index`, `embedding-status`, `context-pack`, `callers`, and `callees` CLI commands
- MCP stdio `initialize`, `tools/list`, and `tools/call` for P0 tools
- MCP tool argument validation with stable JSON-RPC errors
- fixture-based CLI and MCP stdio integration tests
- local smoke scripts for MCP stdio, semantic search, Docker, release install, and benchmark fixtures

Next:

- improve broader TypeScript path alias edge cases and package manager metadata handling

## Install From Release

Install the latest release for the current macOS or Linux platform:

```bash
curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | sh
```

Install a specific version:

```bash
CODEINSIGHT_VERSION=v0.1.9 sh scripts/install.sh
```

Choose a custom install directory:

```bash
INSTALL_DIR="$HOME/bin" sh scripts/install.sh
```

The installer supports:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

For private repositories or rate-limited environments, install and authenticate
GitHub CLI first:

```bash
gh auth login
sh scripts/install.sh
```

Without GitHub CLI, the installer falls back to `curl`. Set `GITHUB_TOKEN` if
the release assets require authentication.

Smoke test the packaged installer path locally:

```bash
scripts/release-install-smoke.sh
```

## Install With Homebrew

Install from the shared Homebrew tap:

```bash
brew tap sleticalboy/tap
brew install codeinsight
```

## Install From Source

```bash
cargo install --path .
```

## Run With Docker

Build and run the local image:

```bash
docker build -t codeinsight:local .
docker run --rm -v "$PWD:/workspace" codeinsight:local overview /workspace
```

Tagged releases publish a GHCR image:

```bash
docker pull ghcr.io/sleticalboy/codeinsight-mcp:latest
docker run --rm -v "$PWD:/workspace" ghcr.io/sleticalboy/codeinsight-mcp:latest overview /workspace
```

Release images are published for `linux/amd64` and `linux/arm64`.

Smoke test the Docker image locally:

```bash
scripts/docker-smoke.sh
CODEINSIGHT_DOCKER_PLATFORM=linux/arm64 scripts/docker-smoke.sh
```

## CLI Usage

Print version information:

```bash
codeinsight version
codeinsight --version
```

Index a repository:

```bash
cargo run -- index /path/to/repo --force
```

Print an overview:

```bash
cargo run -- overview /path/to/repo
```

The overview is the first-stop repository briefing for agents. After `index`, it returns a compact `summary`, language and directory distribution, symbol-kind counts, dependency and call-graph summaries, entrypoint candidates with heuristic reasons, roles, confidence scores, and index metadata.

Search symbols:

```bash
cargo run -- symbols /path/to/repo AuthService
```

Print a file outline:

```bash
cargo run -- outline /path/to/repo/src/auth.ts
```

Print local dependencies:

```bash
cargo run -- dependency-graph /path/to/repo --limit 50
```

Find references:

```bash
cargo run -- find-references /path/to/repo AuthService --include-definitions
```

Build an agent context pack:

```bash
cargo run -- context-pack /path/to/repo --task "understand app entrypoint" --token-budget 6000
cargo run -- context-pack /path/to/repo --task "understand auth flow" --symbol AuthService --token-budget 6000
cargo run -- context-pack /path/to/repo --task "understand auth module" --file src/auth.ts --token-budget 6000
```

Inspect the static call graph:

```bash
cargo run -- callers /path/to/repo helper
cargo run -- callees /path/to/repo AuthService.login
```

Start the MCP stdio server scaffold:

```bash
cargo run -- serve --transport stdio
```

## MCP Tools

The stdio server currently exposes:

- `index_project`
- `project_overview`
- `symbol_search`
- `file_outline`
- `dependency_graph`
- `impact_analysis`
- `find_references`
- `semantic_search`
- `semantic_index`
- `embedding_status`
- `context_pack`
- `callers`
- `callees`

Example `tools/call` request:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"symbol_search","arguments":{"root":"/path/to/repo","query":"AuthService","limit":5}}}
```

For client setup snippets, see [MCP client configuration](docs/mcp-client-config.md).

`project_overview` / `overview` returns the indexed repository briefing an agent should fetch before deeper tools. It preserves the basic file, symbol, language, and top-directory stats, and adds `summary`, `total_lines`, `main_directories`, `symbol_kinds`, `dependency_summary`, `call_summary`, `entrypoints`, and `index_status`. `main_directories` and `entrypoints` include role hints such as `source`, `test`, `fixture`, `vendor`, `docs`, or `example`. Entrypoints are heuristic candidates based on conventional file names and entry-like symbols such as `main`, with a normalized `confidence` score; use `context_pack`, `callers`, `callees`, and `dependency_graph` to inspect the actual flow.

`find_references` is currently a fast text-reference pass over indexed files. It returns file, line, column, context, an approximate reference kind, and a confidence score. Obvious comment-only and string-only matches are filtered before ranking, and test or fixture files are downranked so production references are less likely to be hidden by low-value matches. It is not yet a full language-server-grade semantic reference resolver.

`impact_analysis` estimates a local change radius from seed symbols and/or files. It combines indexed symbol definitions, text references, static callers/callees, and resolved dependency edges into a ranked `impacted_files` list with evidence arrays. Use CLI `impact-analysis /path/to/repo --symbol AuthService --file src/auth.py --depth 2 --format summary` or MCP `impact_analysis` with `symbols`, `files`, `depth`, and `format` after running `index`. Depth expands outward through caller chains and dependency importers, with explanatory `paths` entries for each hop. `format=summary` keeps ranked files and paths while limiting evidence arrays with `evidence_limit`; `format=full` is the default. Reports also include `risk_level`, `impact_counts`, `top_reasons`, and `suggested_checks` so clients can render a compact summary and candidate validation steps without traversing all evidence. Project-specific check commands can be configured in `.codeinsight/config.toml`; run `codeinsight init-config /path/to/repo` to create a sample config with detected test commands. Otherwise built-in command inference is used. See [Impact analysis](docs/impact-analysis.md) for the current scoring and risk-level contract.

`config_status` / `config-status` reports whether `.codeinsight/config.toml` exists, whether it loaded successfully, configured impact-analysis commands, detected fallback test commands, and whether configured commands will override built-in command inference.

`semantic_search` queries local semantic vectors for a configured embedding provider. With `CODEINSIGHT_EMBEDDING_PROVIDER=local-hash`, run `semantic-index` first to build deterministic local vectors, then `semantic-search` ranks chunks by cosine similarity. With `CODEINSIGHT_EMBEDDING_PROVIDER=ollama`, CodeInsight calls a local Ollama `/api/embed` endpoint. With `CODEINSIGHT_EMBEDDING_PROVIDER=openai`, CodeInsight calls an OpenAI-compatible `/embeddings` endpoint and never prints the API key. Without a configured provider, the command returns a clear configuration error instead of silently falling back to lexical search. See [Embedding providers](docs/embedding-providers.md) for the current provider contract and planned external-provider boundary.

`semantic_index` builds local source-text chunks from the existing project index and stores them in SQLite as the local boundary for semantic search. It is deterministic and zero-network by default. Set `CODEINSIGHT_EMBEDDING_PROVIDER=local-hash` to also generate deterministic local embeddings for those chunks. External providers are requested in batches controlled by `CODEINSIGHT_EMBEDDING_BATCH_SIZE`, which defaults to 64 chunks per request. Re-running `semantic_index` preserves embeddings for unchanged chunks and only embeds chunks missing vectors for the selected provider/model. The report keeps total `chunks` and `embeddings` counts and also includes incremental `chunks_added`, `chunks_updated`, `chunks_removed`, `embeddings_generated`, and `embeddings_reused` fields for cache hit visibility. Pass CLI `--explain` or MCP `explain: true` to include per-chunk `changes` entries with add/update/remove ranges and content hashes.

`embedding_status` reports the configured embedding provider, selected model, supported provider names, embedding batch size, Ollama local endpoint settings when selected, and optional semantic chunk/vector counts for a repository. It does not call external services.

`context_pack` combines symbol search, file seeds, reference search, static call graph hints, resolved local dependencies, semantic vector matches when the configured provider has indexed vectors, and local semantic chunk fallback matches into a token-budgeted context bundle for agents. If no `symbols` or `files` are provided, it uses `project_overview` entrypoint candidates to auto-select the highest-confidence `source` entrypoint; if no entrypoint exists, it falls back to indexed source files. Test, fixture, vendor, docs, and example files are not auto-selected unless the task explicitly asks for those roles. The response includes `seed_strategy` (`explicit`, `auto_entrypoint`, or `auto_source_fallback`) and `selected_seeds` so clients can inspect seed decisions without parsing summary text. It ranks candidates before applying the token budget: explicit file seeds first, then symbol definitions, call graph targets, references, semantic matches, and resolved local dependencies, with task keywords used as a lightweight relevance boost. Inferred ranges from test and fixture files are downranked by default, but promoted when the task asks for tests, specs, coverage, regression, or when an explicit seed file is test-like. File seeds include header/import context and primary top-level symbols instead of blindly copying the first chunk of a file; oversized seed ranges can be shortened to fit small budgets. Returned ranges include `source`, `score`, `reason`, and `excerpt`, are trimmed to avoid duplicate lines, and are ordered by source line within each file. File-level `source` and `reason` values identify the dominant selected source, and file-level `score` is the highest selected range score. Source values include `seed_file`, `symbol_definition`, `reference`, `call_graph`, `semantic`, and `dependency`. The `semantic_status` object reports whether vector or fallback semantic ranges were available and selected, plus the next suggested action. The hybrid ranking path remains local-first and falls back cleanly when optional embeddings are not configured.

`callers` and `callees` use a static call graph. Calls include same-file caller/callee names, and obvious JavaScript/TypeScript imported calls can include `callee_file` when local imports, aliases, namespace imports, default imports, and re-exports resolve to indexed files with matching symbols. They are useful navigation signals, not full type-aware call hierarchy results.

For accuracy boundaries and current non-goals, see [Known limitations](docs/known-limitations.md).

## Development

```bash
cargo fmt
cargo test
```

Run benchmark profiles:

```bash
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

Benchmark profiles fail if index times exceed fixture guardrail budgets. To
refresh reports without enforcing budgets, set
`CODEINSIGHT_BENCH_DISABLE_BUDGETS=1`.

## Release Builds

The `Release Build` workflow can be triggered manually or by pushing a `v*` tag. Manual runs build Linux and macOS artifacts and upload them as workflow artifacts. Tag runs also create or update the matching GitHub Release.

## License

CodeInsight MCP Server is licensed under the Apache License 2.0.

The first MVP intentionally avoids external services such as Qdrant, pgvector, Neo4j, or Apache AGE. The default path must remain local, single-binary, and low configuration.
