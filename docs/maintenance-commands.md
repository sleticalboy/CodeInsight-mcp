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
release-tooling smokes, docs smokes, context-pack quality checks, the
agent-router demo, and whitespace diff checks. Nested smoke groups also print
their own numbered stages.

## Maintenance Smoke Groups

Run focused smoke groups when changing the corresponding area:

```bash
scripts/script-syntax-smoke.sh
scripts/workflow-actions-smoke.sh
scripts/benchmark-step-summary-smoke.sh
scripts/context-pack-quality-step-summary-smoke.sh
scripts/docs-smoke.sh
scripts/release-notes-smoke.sh
scripts/release-tooling-smoke.sh
```

The release-tooling smoke prints numbered stages and includes installer
fallback, release verifier diagnostics, release help text, release summary
JSON, release prep, Homebrew formula generation, post-release verification,
status update checks, pretag artifact checks, tagged release workflow guard
checks, and tag preflight smoke.

## Recommended Release Path

Use this as the shortest normal release path. The detailed checklist and
troubleshooting notes live in [Release commands](release-commands.md) and
[Release runbook](release-runbook.md).

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

## Agent And MCP Checks

Run these when changing code routing, context packing, MCP output, or semantic
behavior:

```bash
scripts/two-minute-demo.sh
scripts/agent-router-demo.sh
scripts/demo-output-smoke.sh
scripts/context-pack-quality-smoke.sh
scripts/mcp-stdio-smoke.sh
scripts/semantic-smoke.sh
scripts/installed-quickstart-smoke.sh
```

Use `two-minute-demo.sh` for user-facing `agent_route` walkthroughs and
`agent-router-demo.sh` for lower-level raw metric output and CI-style
assertions. Use `demo-output-smoke.sh` after refreshing
[Demo output snapshot](demo-output.md).

`context-pack-quality-smoke.sh` is a deterministic offline quality regression
check. It uses checked-in and temporary fixtures to verify explicit symbol
seeds, reading-plan suggestions, token-budget metadata, and production-vs-test
reference ranking. It also verifies dependency continuation: a seeded entry
file should pull in its resolved local dependency and recommend a file-scoped
`dependency_graph` follow-up. Low-budget fixtures verify that
`omitted_candidates` and `continuation_summary` expose a bounded follow-up
`context_pack` call, and minimum-budget fixtures verify requests below 500
tokens report `minimum_budget_applied`. Token-exhaustion fixtures verify
`token_budget_exhausted` when selected ranges are truncated without omitted
candidates, all without cloning external repositories. Pass
`--summary-json <path>` to write a machine-readable pass report. Local CI
writes this summary to a temporary file and validates key scenario names; the
remote `context-pack-quality-smoke` job uploads the
`codeinsight-context-pack-quality` JSON artifact and writes the scenario table
to the Actions summary with `scripts/context-pack-quality-step-summary.sh`.

## Benchmark Checks

Run benchmark fixtures when changing indexing, routing, or performance-sensitive
logic:

```bash
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
scripts/benchmark-report-smoke.sh docs/benchmark-v0.1.md smoke
scripts/benchmark-artifact-smoke.sh <ci-run-id>
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
To inspect the artifact locally, download and validate it from a completed
`CI` run:

```bash
scripts/benchmark-artifact-smoke.sh <ci-run-id>
```

To validate the context-pack quality artifact from a completed `CI` run:

```bash
scripts/context-pack-quality-artifact-smoke.sh <ci-run-id>
```

To validate the full release evidence summary against a completed `CI` run,
including both artifact URLs and downloaded local report paths:

```bash
scripts/release-evidence-summary-artifact-smoke.sh --repo sleticalboy/CodeInsight-mcp <ci-run-id>
```

To archive release evidence for a tag after release-prep CI passes:

```bash
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
```

## Optional External Checks

Run these only when the local environment supports the required service or
runtime:

```bash
scripts/docker-smoke.sh
CODEINSIGHT_DOCKER_PLATFORM=linux/arm64 scripts/docker-smoke.sh
scripts/ollama-semantic-smoke.sh
scripts/openai-semantic-smoke.sh
```
