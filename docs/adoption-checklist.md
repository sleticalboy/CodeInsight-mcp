# Adoption Checklist

Use this checklist after following the [Quickstart](quickstart.md). It verifies
that CodeInsight is not just installed, but actually usable as an AI-agent code
context router.

## 1. Binary Is Available

Run:

```bash
codeinsight version
command -v codeinsight
```

Pass criteria:

- `codeinsight version` prints version and target information.
- `command -v codeinsight` prints the same path used in your MCP client config,
  or your client config uses an absolute `command` path.

If this fails, revisit [Install](install.md).

## 2. Local Demo Produces Routing Metrics

Run from a CodeInsight checkout:

```bash
scripts/agent-router-demo.sh
```

Or against your own repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/agent-router-demo.sh
```

Pass criteria:

- `indexed_files` is greater than zero.
- `entrypoints` or `recommended_next_tools` is greater than zero.
- `context_pack` reports `selected_files`, `selected_ranges`, and
  `estimated_tokens`.
- `line_reduction` is present and below 100%.
- `impact_analysis` reports `risk_level`, `impacted_files`, `paths`, or
  `suggested_checks`.

## 3. MCP Server Starts And Lists Tools

Run:

```bash
scripts/mcp-stdio-smoke.sh
```

With an installed binary:

```bash
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/mcp-stdio-smoke.sh
```

Pass criteria:

- Output starts with `MCP stdio smoke passed`.
- `tools` is `15`.
- `indexed_files` is greater than zero.
- `overview_recommendations` is greater than zero.
- `auto_reading_plan_steps` is greater than zero.

If this passes locally but fails inside a GUI client, use an absolute binary
path in the client config. GUI clients often do not inherit shell `PATH`.

## 4. MCP Client Configuration Is Active

Open your MCP client and verify that the `codeinsight` server appears in its
tool list.

Expected tools include:

- `index_project`
- `project_overview`
- `context_pack`
- `impact_analysis`
- `dependency_graph`
- `file_outline`
- `callers`
- `callees`

Pass criteria:

- The client can call `index_project` for a local repository.
- The client can call `project_overview` after indexing.
- `project_overview.recommended_next_tools[]` includes `context_pack`.

See [MCP client configuration](mcp-client-config.md) for Codex, Claude Code,
Cursor, and generic MCP JSON snippets.

## 5. Agent Policy Is Being Followed

Ask your agent:

```text
Use CodeInsight to understand this repository before reading files directly.
Start with project_overview, then build a context_pack for:
"understand the main application entrypoint"
Use a token budget of 6000.
```

Pass criteria:

- The agent calls `project_overview` before broad file reads.
- The agent calls `context_pack` with `root`, `task`, and `token_budget`.
- The agent reads selected files in `reading_plan[]` order.
- The agent does not immediately fall back to broad `rg` / `cat` exploration
  unless CodeInsight points to a file or the user asks for a specific location.

If this fails, place the
[Agent Policy Prompt](client-workflow.md#agent-policy-prompt) in the client's
project instructions.

## 6. Context Pack Demonstrates Token Discipline

Call `context_pack` through CLI or MCP with a realistic task:

```bash
codeinsight index /path/to/repo
codeinsight context-pack /path/to/repo \
  --task "understand the main application entrypoint" \
  --token-budget 6000
```

Pass criteria:

- `budget.applied_token_budget` is set.
- `estimated_tokens` is less than or equal to the applied budget unless the
  response explains truncation.
- `budget.candidate_files` is greater than or equal to
  `budget.selected_files`.
- `reading_plan[]` is present.
- `continuation_summary.status` is present.

When `continuation_summary.status` is `complete`, the agent should read the
selected context before asking for more. When omitted candidates are available,
use `continuation_summary.suggested_tool` only after selected context is read.

## 7. Impact Analysis Runs Before Edits

Before changing a file or symbol, run:

```bash
codeinsight impact-analysis /path/to/repo \
  --file src/main.rs \
  --depth 2 \
  --format summary
```

Pass criteria:

- `risk_level` is present.
- `impact_counts` is present.
- `impacted_files` is present.
- `suggested_checks[]` is present or the response explains why none were
  inferred.

Treat this as edit-planning evidence. CodeInsight is a best-effort local
navigation layer, not a compiler-grade proof engine.

## 8. Benchmark Evidence Is Reproducible

Run:

```bash
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

Pass criteria:

- Both benchmark commands finish without budget or guardrail failures.
- The generated reports include `Key Results`, `Entrypoints`,
  `Recommended tools`, `Line reduction`, `First context file`, and
  `Context pack guardrails`.
- `context_pack` is the first recommended tool for benchmarked repositories.
- `Key Results` summarizes routing, aggregate source-line compression, token
  usage, indexing time, guardrail failures, and truncation status.
- Profile-specific context pack guardrails pass for selected files, selected
  ranges, `reading_plan_steps`, first next action, token budget, and line
  reduction.

Generated reports:

- [Smoke benchmark](benchmark-v0.1.md)
- [Large repository benchmark](benchmark-large.md)

## Adoption Complete

CodeInsight is successfully adopted when:

- The binary works locally.
- The MCP server starts in your client.
- The agent follows the first-read policy.
- `context_pack` returns a bounded reading plan.
- `impact_analysis` is used before edits.
- Local smoke or benchmark evidence can be reproduced on at least one real
  repository.
