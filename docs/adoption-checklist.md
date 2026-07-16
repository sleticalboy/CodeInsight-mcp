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
scripts/two-minute-demo.sh
```

Or against your own repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

Pass criteria:

- `indexed_files` is greater than zero.
- `entrypoints` or `recommended_next_tools` is greater than zero.
- `context_pack` reports `selected_files`, `selected_ranges`, and
  `estimated_tokens`.
- The demo prints `reading_plan_reason` and `selection_reason` for the first
  selected context file.
- `line_reduction` is present and below 100%.
- `impact_analysis` reports `risk_level`, `impacted_files`, `paths`, or
  `suggested_checks`.
- The final talk track names `agent_route` as the default first-read path and
  includes `project_overview`, `context_pack`, and `impact_analysis` as the
  route internals.

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
- `tools` is `16`.
- `indexed_files` is greater than zero.
- `overview_recommendations` is greater than zero.
- `auto_reading_plan_steps` is greater than zero.

If this passes locally but fails inside a GUI client, use an absolute binary
path in the client config. GUI clients often do not inherit shell `PATH`.

## 4. Installed Quickstart Covers The First-Read Route

Run from a CodeInsight checkout after installing `codeinsight`:

```bash
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh
```

Pass criteria:

- Output includes `installed quickstart smoke passed`.
- The smoke covers `version`, `index`, `overview`, `context-pack`,
  CLI `agent-route`, MCP stdio, and MCP `agent_route`.
- `agent_route_tools` includes `index_project`, `project_overview`,
  `context_pack`, and `impact_analysis`.
- `agent_route_impact_status` and `mcp_agent_route_impact_status` are
  `complete`.

This is the user-side end-to-end adoption gate: it proves the installed binary
can route a new agent through the default first-read path outside the source
checkout.

## 5. MCP Client Configuration Is Active

Open your MCP client and verify that the `codeinsight` server appears in its
tool list.

Expected tools include:

- `agent_route`
- `index_project`
- `project_overview`
- `context_pack`
- `impact_analysis`
- `dependency_graph`
- `file_outline`
- `callers`
- `callees`

Pass criteria:

- The client can call `agent_route` for the default first-read path.
- `agent_route.route[]` includes `index_project`, `project_overview`,
  `context_pack`, and `impact_analysis`.
- `agent_route.context_pack.reading_plan[]` is present.
- The client can call `index_project` for a local repository.
- The client can call `project_overview` after indexing when step-by-step
  routing is needed.
- `project_overview.recommended_next_tools[]` includes `context_pack`.

See [MCP client configuration](mcp-client-config.md) for Codex, Claude Code,
Cursor, and generic MCP JSON snippets.

## 6. Agent Policy Is Being Followed

Ask your agent:

```text
Use CodeInsight to understand this repository before reading files directly.
Start with agent_route for:
"understand the main application entrypoint"
Use a token budget of 6000.
```

Pass criteria:

- The agent calls `agent_route` with `root`, `task`, and `token_budget` before
  broad file reads.
- The agent uses `project_overview` and `context_pack` from the `agent_route`
  response instead of duplicating the first-read path manually.
- The agent reads selected files in `reading_plan[]` order and uses
  `reading_plan[].reason` as the current-step instruction.
- The agent can explain `reading_plan[].selection_reason` as the evidence for
  why a file was selected, without treating it as a replacement for
  `reading_plan[].reason`.
- The agent does not execute `continuation_summary.suggested_tool` or
  `omitted_candidates[].suggested_tool` until the selected `files[]` excerpts
  have been read.
- The agent does not immediately fall back to broad `rg` / `cat` exploration
  unless CodeInsight points to a file or the user asks for a specific location.

If this fails, place the
[Agent Policy Prompt](client-workflow.md#agent-policy-prompt) in the client's
project instructions.

## 7. Context Pack Demonstrates Token Discipline

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
- `reading_plan[0].reason` is present and explains the question, deeper
  evidence tool, and selection rationale.
- `reading_plan[0].selection_reason` is present and explains why the first file
  was selected.
- `continuation_summary.status` is present.

When `continuation_summary.status` is `complete`, the agent should read the
selected context before asking for more. When omitted candidates are available,
use `continuation_summary.suggested_tool` only after selected context is read.

## 8. Impact Analysis Runs Before Edits

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

## 9. Benchmark Evidence Is Reproducible

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
- The installed quickstart smoke covers CLI `agent-route` and MCP
  `agent_route`.
- The agent follows the `agent_route` first-read policy.
- `context_pack` returns a bounded reading plan.
- The agent follows `reading_plan[].reason`, can surface
  `reading_plan[].selection_reason`, and waits to use continuation tools until
  selected context has been read.
- `impact_analysis` is used before edits.
- Local smoke or benchmark evidence can be reproduced on at least one real
  repository.
