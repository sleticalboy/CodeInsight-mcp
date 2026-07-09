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
- [Install](docs/install.md)
- [CLI usage](docs/cli-usage.md)
- [Navigation tools](docs/navigation-tools.md)
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

Install the latest macOS or Linux release:

```bash
curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | sh
```

For version pinning, custom install directories, authenticated downloads, and
installer smoke tests, see [Install](docs/install.md).

## Install With Homebrew

```bash
brew tap sleticalboy/tap
brew install codeinsight
```

## Install From Source

```bash
cargo install --path .
```

## Run With Docker

```bash
docker pull ghcr.io/sleticalboy/codeinsight-mcp:latest
docker run --rm -v "$PWD:/workspace" ghcr.io/sleticalboy/codeinsight-mcp:latest overview /workspace
```

For local image builds, platform details, and Docker smoke tests, see
[Install](docs/install.md).

## CLI Usage

Index a repository, inspect the overview, then build an agent context pack:

```bash
cargo run -- index /path/to/repo --force
cargo run -- overview /path/to/repo
cargo run -- context-pack /path/to/repo --task "understand app entrypoint" --token-budget 6000
```

Start the MCP stdio server:

```bash
cargo run -- serve --transport stdio
```

For all commands and common workflows, see [CLI usage](docs/cli-usage.md).

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

Recommended MCP first-read flow:

1. `index_project` for the repository.
2. `project_overview` to inspect summary, roles, and entrypoint candidates.
3. `context_pack` with `root`, `task`, and `token_budget`; omit `symbols` and
   `files` to let CodeInsight auto-select the highest-confidence source
   entrypoint.

For the full first-read contract, see [First-read workflow](docs/first-read-workflow.md). For client setup snippets, see [MCP client configuration](docs/mcp-client-config.md).

`project_overview` / `overview` returns the indexed repository briefing an agent should fetch before deeper tools, including role-aware directories, entrypoint candidates, summaries, index metadata, and `recommended_next_tools`. See [Recommendation contract](docs/recommendation-contract.md) for the shared recommendation shape.

`find_references` is a fast text-reference pass over indexed files. It returns ranked file, location, context, approximate reference kind, and confidence entries, with obvious comment/string matches filtered and test or fixture paths downranked. See [Navigation tools](docs/navigation-tools.md) for CLI/MCP usage and response fields.

`impact_analysis` estimates a local change radius from seed symbols and/or files using definitions, text references, static calls, and resolved local dependencies. It returns ranked impacted files, paths, risk level, top reasons, and suggested checks. See [Impact analysis](docs/impact-analysis.md) for CLI/MCP usage, scoring, risk levels, and validation-command configuration.

`config_status` / `config-status` reports whether `.codeinsight/config.toml` exists, whether it loaded successfully, configured impact-analysis commands, detected fallback test commands, and whether configured commands will override built-in command inference.

`semantic_search`, `semantic_index`, and `embedding_status` provide the optional semantic search path. Semantic indexing is local and zero-network by default, embeddings are generated only when a provider is configured, and `embedding_status` reports provider/index state without making network calls. See [Embedding providers](docs/embedding-providers.md) for provider setup, batching, incremental indexing counters, explain output, and external-provider boundaries.

`context_pack` returns a token-budgeted, agent-ready context bundle from explicit seeds or inferred source entrypoints. It includes selected files and ranges, `seed_strategy`, `selected_seeds`, `reading_plan`, semantic status, and prioritized follow-up tool suggestions. See [First-read workflow](docs/first-read-workflow.md) for the full ranking and response contract.

`callers` and `callees` use a static call graph. They return same-file call edges and best-effort imported target hints such as JavaScript/TypeScript `callee_file` values when local import/export paths resolve to indexed symbols. See [Navigation tools](docs/navigation-tools.md) for supported hints and boundaries.

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

The `Release Build` workflow supports manual artifact builds and tagged GitHub
releases. See [Release runbook](docs/release-runbook.md).

## License

CodeInsight MCP Server is licensed under the Apache License 2.0.

The first MVP intentionally avoids external services such as Qdrant, pgvector, Neo4j, or Apache AGE. The default path must remain local, single-binary, and low configuration.
