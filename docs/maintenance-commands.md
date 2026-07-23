# Maintenance Commands

Use this as the short command index for local development and maintenance
smoke checks. For tagged release commands, see [Release commands](release-commands.md).

## Local Development

Run the standard non-network local gate:

```bash
scripts/local-ci-smoke.sh
```

This prints numbered stages and runs formatting, Rust tests, clippy, shell
syntax checks, workflow action version checks, benchmark, context-pack, and
agent-route step-summary checks, release-tooling smokes, docs smokes,
context-pack quality checks, the `agent_route` contract smoke, the
agent-router demo, and whitespace diff checks. Nested smoke groups also print
their own numbered stages.

## Maintenance Smoke Groups

Run focused smoke groups when changing the corresponding area:

```bash
scripts/script-syntax-smoke.sh
scripts/clippy-smoke.sh
scripts/workflow-actions-smoke.sh
scripts/benchmark-step-summary-smoke.sh
scripts/context-pack-quality-step-summary-smoke.sh
scripts/agent-route-step-summary-smoke.sh
scripts/mcp-first-call-step-summary-smoke.sh
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
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp --evidence-file release-evidence/vX.Y.Z.md --evidence-json-file release-evidence/vX.Y.Z.json vX.Y.Z main
scripts/prepare-release.sh --dry-run vX.Y.Z
scripts/prepare-release.sh vX.Y.Z
git push origin main
scripts/release-pretag-check.sh main
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX.Y.Z.json vX.Y.Z main
scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
scripts/post-release-verify.sh --handoff vX.Y.Z
```

## Agent And MCP Checks

Run these when changing code routing, context packing, MCP output, or semantic
behavior:

```bash
scripts/two-minute-demo.sh
scripts/agent-route-smoke.sh
scripts/agent-router-demo.sh
scripts/demo-output-smoke.sh
scripts/context-pack-quality-smoke.sh
scripts/mcp-first-call-smoke.sh
scripts/mcp-first-call-failure-smoke.sh
scripts/mcp-stdio-smoke.sh
scripts/semantic-smoke.sh
scripts/installed-quickstart-smoke.sh
scripts/public-task-routing-matrix-smoke.sh
scripts/update-public-task-routing-matrix-smoke.sh
scripts/competitive-routing-smoke.sh
```

Choose the narrowest check for the change:

| Change Or Question | Command | Scope |
| --- | --- | --- |
| README/demo positioning changed | `scripts/two-minute-demo.sh` and `scripts/demo-output-smoke.sh` | User-facing `agent_route` walkthrough and checked snapshot. |
| First MCP call onboarding changed | `scripts/mcp-first-call-smoke.sh --summary-json /tmp/codeinsight-mcp-first-call.json` | Compact JSON proof for `agent_route`, first context file, task-path seed evidence, read-less metrics, selection rank, reading-question handoff, continuation summary, reading-plan order, suggested-tool handoff, impact status, blocked no-seed/no-context/unindexed-path handling, and saved artifacts. |
| First MCP call Actions summary changed | `scripts/mcp-first-call-step-summary-smoke.sh` | Checks the Actions Summary section for selected files, task-path seed evidence, first context file, first reading file, read-less metrics, selection rank, reading-question handoff, omitted-candidate continuation fields, reading-plan order, suggested-tool handoff, continuation timing, impact status, blocked no-seed/no-context/unindexed-path handling, and artifact link. |
| First MCP call help or failure messaging changed | `scripts/mcp-first-call-failure-smoke.sh` | Fast checks for `--help`, `[usage]`, `[binary]`, and `[mcp_server]` output. |
| MCP protocol or tool payload changed | `scripts/mcp-stdio-smoke.sh` | Stdio MCP handshake, `agent_route`, `context_pack`, executable suggested-tool calls, read-less metrics, selection rank, and continuation evidence. |
| Framework entrypoint routing changed | `scripts/framework-entrypoint-demo.sh` | Temporary multi-framework fixture covering Next.js, Rails, Django, and C# web first-context selection. |
| Task alias or seed ordering changed | `scripts/task-routing-matrix-smoke.sh` | Temporary fixture proving routing, authentication, authorization, access-control, settings, feature flag, network, TLS, validation, startup, persistence, debug, coverage, API handler, cache, observability, security, billing, frontend, background job, documentation, request lifecycle, middleware, and AI-agent first-read prompts choose the matching first file and that `--expect-file` failures are reported. |
| Public route expectations changed | `scripts/public-task-routing-matrix.sh --case express --root express=/path/to/express` | Checked-in public repository expectation files aggregated into one route-quality summary. Defaults include pinned Express, FastAPI, Flask, Gin, Requests, and Streamlit cases. Use `scripts/public-task-routing-matrix-smoke.sh` for a deterministic no-network contract check. |
| Public route snapshot changed | `scripts/update-public-task-routing-matrix.sh --check` | Checked-in public route-quality snapshot freshness. Use `scripts/update-public-task-routing-matrix-smoke.sh` for a deterministic no-network contract check. |
| Competitive positioning changed | `scripts/competitive-routing-smoke.sh` | Deterministic no-network scaffold for comparing CodeInsight's agent first-read route quality against generic code-memory tools without requiring the competitor to be installed. |
| Installed-binary adoption path changed | `CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh` | CLI and MCP first-read routes through the installed binary, including read-less metrics, selection rank, and continuation evidence. |
| One-call `agent_route` JSON contract changed | `scripts/agent-route-smoke.sh` | Route order, execution plan, context pack, and impact-analysis preview. |
| Context ranking or continuation changed | `scripts/context-pack-quality-smoke.sh` | Deterministic context-pack quality regressions. |

Use `two-minute-demo.sh` for user-facing `agent_route` walkthroughs,
`agent-route-smoke.sh` for the one-call JSON contract, and
`agent-router-demo.sh` for lower-level metrics, reading reasons, impact
breakdown output, and CI-style assertions. Use
`framework-entrypoint-demo.sh` when entrypoint heuristics or task matching
changes touch framework routing. Use
`task-routing-matrix.sh --expect-file route-expectations.tsv` for real
repository multi-prompt route-quality gates, `public-task-routing-matrix.sh`
when refreshing all checked-in public route expectation cases, and
`update-public-task-routing-matrix.sh` when refreshing the checked-in public
snapshot. Use `task-routing-matrix-smoke.sh` for deterministic alias/seed-order
regressions. Use `competitive-routing-smoke.sh` when updating
[codebase-memory-mcp comparison](competitive-analysis-codebase-memory.md) or
other competitive positioning docs.
Use `demo-output-smoke.sh` after refreshing [Demo output snapshot](demo-output.md).

`agent-route-smoke.sh --summary-json <path>` writes a reusable JSON evidence
summary for the one-call first-read route. The remote `agent-route-smoke` job
uploads it as `codeinsight-agent-route-smoke` and writes the key route,
context, first reading focus/question, selection rank, continuation next action,
token-budget, and impact metrics to the Actions summary with
`scripts/agent-route-step-summary.sh`.

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
to the Actions summary with `scripts/context-pack-quality-step-summary.sh`,
including first reading focus/question metrics for selected context and a question
coverage table for seed-file, call-graph, reference, dependency, and semantic
reading-plan actions.

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
compact JSON summary, report `Key Results`, summary table, workflow run link,
and artifact link into the run summary with `scripts/benchmark-step-summary.sh`,
so maintainers can inspect the routing evidence before downloading the full
artifact. Benchmark reports include a `Key Results` section for stable README,
release note, and demo evidence.
To inspect the artifact locally, download and validate it from a completed
`CI` run:

```bash
scripts/benchmark-artifact-smoke.sh <ci-run-id>
```

The artifact smoke validates both the Markdown report and the compact JSON
summary uploaded by `benchmark-subset-smoke`.

To validate the context-pack quality artifact from a completed `CI` run:

```bash
scripts/context-pack-quality-artifact-smoke.sh <ci-run-id>
```

To validate the one-call agent-route artifact from a completed `CI` run:

```bash
scripts/agent-route-artifact-smoke.sh <ci-run-id>
```

To validate the first MCP call artifact from a completed `CI` run:

```bash
scripts/mcp-first-call-artifact-smoke.sh <ci-run-id>
```

To validate the full release evidence summary against a completed `CI` run,
including artifact URLs and downloaded local report paths:

```bash
scripts/release-evidence-summary-artifact-smoke.sh --repo sleticalboy/CodeInsight-mcp <ci-run-id>
```

To archive release evidence for a tag after release-prep CI passes:

```bash
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX.Y.Z.json vX.Y.Z main
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
