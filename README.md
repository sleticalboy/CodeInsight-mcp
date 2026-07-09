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

Recommended MCP first-read flow:

1. `index_project` for the repository.
2. `project_overview` to inspect summary, roles, and entrypoint candidates.
3. `context_pack` with `root`, `task`, and `token_budget`; omit `symbols` and
   `files` to let CodeInsight auto-select the highest-confidence source
   entrypoint.

For the full first-read contract, see [First-read workflow](docs/first-read-workflow.md). For client setup snippets, see [MCP client configuration](docs/mcp-client-config.md).

`project_overview` / `overview` returns the indexed repository briefing an agent should fetch before deeper tools, including role-aware directories, entrypoint candidates, summaries, index metadata, and `recommended_next_tools`. See [Recommendation contract](docs/recommendation-contract.md) for the shared recommendation shape.

`find_references` is currently a fast text-reference pass over indexed files. It returns file, line, column, context, an approximate reference kind, and a confidence score. Obvious comment-only and string-only matches are filtered before ranking, and test or fixture files are downranked so production references are less likely to be hidden by low-value matches. It is not yet a full language-server-grade semantic reference resolver.

`impact_analysis` estimates a local change radius from seed symbols and/or files. It combines indexed symbol definitions, text references, static callers/callees, and resolved dependency edges into a ranked `impacted_files` list with evidence arrays. Use CLI `impact-analysis /path/to/repo --symbol AuthService --file src/auth.py --depth 2 --format summary` or MCP `impact_analysis` with `symbols`, `files`, `depth`, and `format` after running `index`. Depth expands outward through caller chains and dependency importers, with explanatory `paths` entries for each hop. `format=summary` keeps ranked files and paths while limiting evidence arrays with `evidence_limit`; `format=full` is the default. Reports also include `risk_level`, `impact_counts`, `top_reasons`, and `suggested_checks` so clients can render a compact summary and candidate validation steps without traversing all evidence. Project-specific check commands can be configured in `.codeinsight/config.toml`; run `codeinsight init-config /path/to/repo` to create a sample config with detected test commands. Otherwise built-in command inference is used. See [Impact analysis](docs/impact-analysis.md) for the current scoring and risk-level contract.

`config_status` / `config-status` reports whether `.codeinsight/config.toml` exists, whether it loaded successfully, configured impact-analysis commands, detected fallback test commands, and whether configured commands will override built-in command inference.

`semantic_search`, `semantic_index`, and `embedding_status` provide the optional semantic search path. Semantic indexing is local and zero-network by default, embeddings are generated only when a provider is configured, and `embedding_status` reports provider/index state without making network calls. See [Embedding providers](docs/embedding-providers.md) for provider setup, batching, incremental indexing counters, explain output, and external-provider boundaries.

`context_pack` returns a token-budgeted, agent-ready context bundle from explicit seeds or inferred source entrypoints. It includes selected files and ranges, `seed_strategy`, `selected_seeds`, `reading_plan`, semantic status, and prioritized follow-up tool suggestions. See [First-read workflow](docs/first-read-workflow.md) for the full ranking and response contract.

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
