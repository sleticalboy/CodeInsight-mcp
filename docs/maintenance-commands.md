# Maintenance Commands

Use this as the short command index for local development and maintenance
smoke checks. For tagged release commands, see [Release commands](release-commands.md).

## Local Development

Run the standard non-network local gate:

```bash
scripts/local-ci-smoke.sh
```

This prints numbered stages and runs formatting, Rust tests, shell syntax
checks, release-tooling smokes, docs smokes, and whitespace diff checks.

## Maintenance Smoke Groups

Run focused smoke groups when changing the corresponding area:

```bash
scripts/script-syntax-smoke.sh
scripts/docs-smoke.sh
scripts/release-notes-smoke.sh
scripts/release-tooling-smoke.sh
```

The release-tooling smoke includes installer fallback, release verifier
diagnostics, release help text, release summary JSON, release prep, Homebrew
formula generation, post-release verification, and status update checks.

## Agent And MCP Checks

Run these when changing code routing, context packing, MCP output, or semantic
behavior:

```bash
scripts/agent-router-demo.sh
scripts/mcp-stdio-smoke.sh
scripts/semantic-smoke.sh
scripts/installed-quickstart-smoke.sh
```

## Benchmark Checks

Run benchmark fixtures when changing indexing, routing, or performance-sensitive
logic:

```bash
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

Benchmark profiles enforce fixture guardrail budgets by default. To refresh
reports without enforcing budgets, set `CODEINSIGHT_BENCH_DISABLE_BUDGETS=1`.

## Optional External Checks

Run these only when the local environment supports the required service or
runtime:

```bash
scripts/docker-smoke.sh
CODEINSIGHT_DOCKER_PLATFORM=linux/arm64 scripts/docker-smoke.sh
scripts/ollama-semantic-smoke.sh
scripts/openai-semantic-smoke.sh
```
