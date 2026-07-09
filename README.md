# CodeInsight MCP Server

CodeInsight MCP Server is a local-first code intelligence layer for AI coding agents.

The MVP focuses on a narrow, verifiable loop:

- index a local repository
- extract source symbols with Tree-sitter
- search symbols from a local SQLite index
- expose an MCP stdio server scaffold
- build toward agent-ready context packs

Key docs:

- [Current status](docs/status.md)
- [Install](docs/install.md)
- [CLI usage](docs/cli-usage.md)
- [MCP tools](docs/mcp-tools.md)
- [Known limitations](docs/known-limitations.md)
- [Documentation index](docs/README.md)

## Current Status

CodeInsight is an early MVP. It can index local repositories, expose CLI and
MCP navigation tools, build agent-ready context packs, run local impact
analysis, and optionally use configured semantic embeddings.

Next focus: broaden package manager metadata and workspace edge-case handling.
See [Current status](docs/status.md) for the full implemented capability list.

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

Recommended MCP first-read flow:

1. `index_project` for the repository.
2. `project_overview` to inspect summary, roles, and entrypoint candidates.
3. `context_pack` with `root`, `task`, and `token_budget`; omit `symbols` and
   `files` to let CodeInsight auto-select the highest-confidence source
   entrypoint.

For the full tool list, `tools/call` examples, topic contracts, and accuracy
boundaries, see [MCP tools](docs/mcp-tools.md). For client setup snippets, see
[MCP client configuration](docs/mcp-client-config.md).

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
