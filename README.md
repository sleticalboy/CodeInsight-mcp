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
- [Smoke benchmark](docs/benchmark-v0.1.md)
- [Large repository benchmark](docs/benchmark-large.md)
- [Changelog](CHANGELOG.md)

## Current Status

This repository is an early MVP scaffold. It is not yet a complete MCP code-analysis server.

Implemented:

- Rust CLI entrypoint
- local SQLite index cache under `.codeinsight/`
- incremental indexing with file-hash skips and stale file cleanup
- index metadata with schema and index version tracking
- per-file indexing errors in reports without aborting the whole project scan
- Tree-sitter parsing for TypeScript/JavaScript, Python, Go, Rust, Java, C, and C++
- symbol extraction for common declarations
- dependency graph, text reference search, context packs, and call graph tools with imported target hints
- relative file resolution for local dependency graph edges
- `index`, `overview`, `symbols`, `outline`, `dependency-graph`, `find-references`, `context-pack`, `callers`, and `callees` CLI commands
- MCP stdio `initialize`, `tools/list`, and `tools/call` for P0 tools
- MCP tool argument validation with stable JSON-RPC errors
- fixture-based CLI and MCP stdio integration tests

Next:

- improve broader TypeScript path alias edge cases and package manager metadata handling

## Install From Release

Install the latest release for the current macOS or Linux platform:

```bash
curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | sh
```

Install a specific version:

```bash
CODEINSIGHT_VERSION=v0.1.0 sh scripts/install.sh
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

Install directly from the formula in this repository:

```bash
brew tap sleticalboy/codeinsight-mcp https://github.com/sleticalboy/CodeInsight-mcp
brew install sleticalboy/codeinsight-mcp/codeinsight
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

Smoke test the Docker image locally:

```bash
scripts/docker-smoke.sh
```

## CLI Usage

Index a repository:

```bash
cargo run -- index /path/to/repo --force
```

Print an overview:

```bash
cargo run -- overview /path/to/repo
```

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
- `find_references`
- `context_pack`
- `callers`
- `callees`

Example `tools/call` request:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"symbol_search","arguments":{"root":"/path/to/repo","query":"AuthService","limit":5}}}
```

For client setup snippets, see [MCP client configuration](docs/mcp-client-config.md).

`find_references` is currently a fast text-reference pass over indexed files. It returns file, line, column, context, an approximate reference kind, and a confidence score. It is not yet a full language-server-grade semantic reference resolver.

`context_pack` combines symbol search, file seeds, reference search, static call graph hints, and resolved local dependencies into a token-budgeted context bundle for agents. It ranks candidates before applying the token budget: explicit file seeds first, then symbol definitions, call graph targets, references, and resolved local dependencies, with task keywords used as a lightweight relevance boost. File seeds include header/import context and primary top-level symbols instead of blindly copying the first chunk of a file; oversized seed ranges can be shortened to fit small budgets. Returned ranges include `reason` and `excerpt`, are trimmed to avoid duplicate lines, and are ordered by source line within each file. The first version is deterministic and local-only; it does not use embeddings.

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
