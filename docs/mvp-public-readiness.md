# MVP Public Readiness

This page is the public-showcase gate for the current MVP positioning:
CodeInsight is a local-first MCP code context router for AI coding agents.

Use it before a README recording, launch post, demo call, or open-source
announcement. It is narrower than a tagged release checklist: it verifies that
the MVP story is understandable, demonstrable, and backed by route-quality
evidence.

## Go Criteria

- README states the narrow product position and does not claim IDE, LSP,
  compiler, or Sourcegraph replacement.
- A new user can follow Quickstart from install to a local stdio MCP server.
- The primary workflow is visible:
  `agent_route -> selected context -> executable suggested_tool -> impact check`.
- Demo scripts can produce a short first-read route with read-less evidence.
- At least one real public-repository route-quality matrix is available.
- Local smoke checks pass from the target commit.
- GitHub Actions `CI` is green for the target commit before publishing.
- Known limitations are reachable from the README path.

## Public Demo Path

Run the shortest product walkthrough:

```bash
scripts/two-minute-demo.sh
```

The demo should show:

- indexed files and symbols
- `agent_route` as the single first-read entrypoint
- selected files and reading-plan steps
- `context_pack.read_less` source-line reduction evidence
- an executable suggested tool after selected context
- an impact-analysis preview before edits

For MCP client wiring, run:

```bash
scripts/mcp-stdio-smoke.sh
scripts/mcp-first-call-smoke.sh
```

For an installed-binary adoption gate, run:

```bash
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh
```

## Route-Quality Evidence

Checked-in public matrix:

```bash
scripts/update-public-task-routing-matrix.sh --check
```

Current checked-in snapshot:

- Repositories: Express, FastAPI, Flask, Gin, Requests, and Streamlit.
- Expected first-file checks: `86/86`.
- Selected lines: `41,455` of `7,098,531` task source lines.
- Aggregate first-read line reduction: `99.41%`.
- Output:
  [public-task-routing-matrix.md](public-task-routing-matrix.md) and
  [public-task-routing-matrix-summary.json](public-task-routing-matrix-summary.json).

Heavyweight manual probe:

```bash
scripts/public-task-routing-matrix.sh --case django
```

Latest local Django probe:

- Tasks: URL resolver routing, request/response lifecycle, middleware behavior.
- Expected first-file checks: `3/3`.
- Aggregate first-read line reduction: `99.87%`.

Treat these numbers as route-quality and first-read discipline evidence, not as
runtime performance or proof that unselected code is irrelevant.

## Local Verification

Minimum local gate for public MVP work:

```bash
cargo test --locked
scripts/docs-smoke.sh
scripts/local-ci-smoke.sh
scripts/mcp-stdio-smoke.sh
scripts/public-task-routing-matrix-smoke.sh
git diff --check
```

When routing behavior changes, also rerun the relevant public route matrix case
and update the checked-in snapshot if the default matrix output changes.

## GitHub Actions Gate

Before publishing, confirm the latest `CI` run for the target commit is green:

```bash
gh run list --workflow CI --branch main --limit 5
gh run view <run-id> --json status,conclusion,url,headSha,jobs
```

If all jobs are still queued on GitHub-hosted runners, treat that as an external
execution wait, not a local pass. Do not announce the MVP as verified until the
target commit has a completed successful run or the CI result is verified in the
GitHub Actions UI.

## Public Messaging Guardrails

Say:

- Local-first MCP context router for AI coding agents.
- Designed to reduce blind first-read repository scanning.
- Best for routing, reading plans, context selection, and pre-edit impact
  planning.

Do not say:

- Compiler-grade static analysis.
- IDE or LSP replacement.
- Enterprise collaboration platform.
- Default semantic search quality without a configured embedding provider.
