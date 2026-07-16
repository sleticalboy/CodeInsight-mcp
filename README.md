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
entrypoint and recommendation counts, selected context size, reading-plan
steps, line-reduction percentage, continuation status, and impact-analysis
summary.

For a recording or project introduction, use the
[two-minute demo script](docs/demo-script.md).

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

- Smoke repositories route `context_pack` first for 4/4 repositories and
  select 629 of 75,753 source lines, a 99.2% aggregate line reduction.
- Large repositories route `context_pack` first for 4/4 repositories and
  select 1,748 of 241,555 source lines, a 99.3% aggregate line reduction.
- Generated reports include a `Key Results` section with routing,
  compression, token-budget, indexing, guardrail, and truncation evidence.

Refresh the reports locally:

```bash
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

Key docs:

- [Quickstart](docs/quickstart.md)
- [Adoption checklist](docs/adoption-checklist.md)
- [Demo script](docs/demo-script.md)
- [Agent prompt templates](docs/agent-prompt-template.md)
- [Current status](docs/status.md)
- [Maintainer checklist](docs/maintainer-checklist.md)
- [Maintenance commands](docs/maintenance-commands.md)
- [Install](docs/install.md)
- [CLI usage](docs/cli-usage.md)
- [MCP tools](docs/mcp-tools.md)
- [First-read workflow](docs/first-read-workflow.md)
- [Client workflow](docs/client-workflow.md)
- [Release commands](docs/release-commands.md)
- [Release readiness](docs/release-readiness.md)
- [Known limitations](docs/known-limitations.md)
- [Documentation index](docs/README.md)

## Current Status

CodeInsight is an early MVP. It can index local repositories, expose CLI and
MCP navigation tools, build agent-ready context packs, run local impact
analysis, and optionally use configured semantic embeddings.

Latest verified release: `v0.1.12`.

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

For the full install-to-agent path, see [Quickstart](docs/quickstart.md).

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
scripts/local-ci-smoke.sh
```

For focused smoke groups, benchmark checks, and optional external checks, see
[Maintenance commands](docs/maintenance-commands.md).

## Release Builds

The `Release Build` workflow supports manual artifact builds and tagged GitHub
releases. See [Release runbook](docs/release-runbook.md).

Maintainer release path:

```bash
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp --evidence-file release-evidence/vX.Y.Z.md vX.Y.Z main
scripts/prepare-release.sh --dry-run vX.Y.Z
scripts/prepare-release.sh vX.Y.Z
git push origin main
scripts/release-pretag-check.sh main
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
scripts/post-release-verify.sh vX.Y.Z
```

## License

CodeInsight MCP Server is licensed under the Apache License 2.0.

The first MVP intentionally avoids external services such as Qdrant, pgvector, Neo4j, or Apache AGE. The default path must remain local, single-binary, and low configuration.
