# Maintenance Commands

Use this as the short command index for local development and maintenance
smoke checks. For tagged release commands, see [Release commands](release-commands.md).

## Local Development

Run the standard non-network local gate:

```bash
scripts/local-ci-smoke.sh
```

This prints numbered stages and runs formatting, Rust tests, shell syntax
checks, workflow action version checks, benchmark step-summary checks,
release-tooling smokes, docs smokes, the agent-router demo, and whitespace diff
checks. Nested smoke groups also print their own numbered stages.

## Maintenance Smoke Groups

Run focused smoke groups when changing the corresponding area:

```bash
scripts/script-syntax-smoke.sh
scripts/workflow-actions-smoke.sh
scripts/benchmark-step-summary-smoke.sh
scripts/docs-smoke.sh
scripts/release-notes-smoke.sh
scripts/release-tooling-smoke.sh
```

The release-tooling smoke prints numbered stages and includes installer
fallback, release verifier diagnostics, release help text, release summary
JSON, release prep, Homebrew formula generation, post-release verification,
and status update checks.

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
scripts/benchmark-report-smoke.sh docs/benchmark-v0.1.md smoke
```

Benchmark profiles enforce fixture guardrail budgets by default. To refresh
reports without enforcing budgets, set `CODEINSIGHT_BENCH_DISABLE_BUDGETS=1`.
When public GitHub cloning is unstable, set `CODEINSIGHT_BENCH_REUSE_REPOS=1`
to reuse existing checkouts in the benchmark work directory.
To rerun only specific fixtures, set `CODEINSIGHT_BENCH_REPOS=p-limit,memchr`.
Subset runs write to the benchmark work directory unless
`CODEINSIGHT_BENCH_OUTPUT` is set explicitly.
CI runs a lightweight benchmark subset for `p-limit`; full smoke and large
benchmark reports remain local maintenance checks. The CI subset report is
validated with `scripts/benchmark-report-smoke.sh` and uploaded as the
`codeinsight-benchmark-subset` workflow artifact. CI also writes the subset
report `Key Results`, summary table, workflow run link, and artifact link into
the run summary with `scripts/benchmark-step-summary.sh`, so maintainers can
inspect the routing evidence before downloading the full artifact. Benchmark
reports include a `Key Results` section for stable README, release note, and
demo evidence.

## Optional External Checks

Run these only when the local environment supports the required service or
runtime:

```bash
scripts/docker-smoke.sh
CODEINSIGHT_DOCKER_PLATFORM=linux/arm64 scripts/docker-smoke.sh
scripts/ollama-semantic-smoke.sh
scripts/openai-semantic-smoke.sh
```
