# CodeInsight MCP Server

CodeInsight MCP Server is a local-first code context router for AI coding
agents.

It is built for the first-read problem: before an agent edits a repository, it
needs to know which files matter, what entrypoints to inspect, what context fits
the token budget, and what may be impacted by a change. CodeInsight keeps that
loop local and exposes it through CLI and MCP tools.

The primary agent loop is:

1. `index_project` to build a local SQLite index.
2. `project_overview` to identify entrypoints, directory roles, and next tools.
3. `context_pack` to select a token-budgeted reading set.
4. `impact_analysis` to estimate change radius before edits.

CodeInsight is not trying to replace an IDE, LSP, compiler, or Sourcegraph. Its
job is to help AI agents read less code, pick better context, and stop guessing
which file to open next.

## Two-Minute Demo

Run the local agent-router demo against this repository:

```bash
scripts/agent-router-demo.sh
```

Run it against another repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/agent-router-demo.sh
```

The demo executes the same product path an MCP client should follow:
`index -> overview -> context-pack -> impact-analysis`. It prints index timing,
entrypoint and recommendation counts, selected context size, line-reduction
percentage, continuation status, and impact-analysis summary.

## Current Evidence

The checked-in benchmark reports exercise real public repositories and measure
how much source text `context_pack` keeps under a 6000-token budget, plus the
entrypoint and recommended-tool routing decisions from `project_overview`:

- [Smoke benchmark](docs/benchmark-v0.1.md): p-limit, itsdangerous, Go example,
  and memchr.
- [Large repository benchmark](docs/benchmark-large.md): express, Flask, Gin,
  and Tokio.

These are fixture reports, not controlled performance claims, but they verify
that CodeInsight can index real repositories and return bounded context packs
without external services.

Current benchmark snapshot:

- Smoke repositories select 89-242 context lines from 1,123-69,381 indexed
  lines, a 85.9%-99.7% line reduction.
- Large repositories select 305-573 context lines from 18,337-177,479 indexed
  lines, a 96.9%-99.7% line reduction.
- Every benchmarked repository returns four recommended next tools, with
  `context_pack` as the first recommended action.

Refresh the reports locally:

```bash
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

Key docs:

- [Current status](docs/status.md)
- [Install](docs/install.md)
- [CLI usage](docs/cli-usage.md)
- [MCP tools](docs/mcp-tools.md)
- [First-read workflow](docs/first-read-workflow.md)
- [Known limitations](docs/known-limitations.md)
- [Documentation index](docs/README.md)

## Current Status

CodeInsight is an early MVP. It can index local repositories, expose CLI and
MCP navigation tools, build agent-ready context packs, run local impact
analysis, and optionally use configured semantic embeddings.

Next focus: tighten the README/demo/benchmark path around the AI-agent
first-read workflow.
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
