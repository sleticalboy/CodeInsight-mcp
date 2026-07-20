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

When you need a complete adoption evidence bundle for your own repository:

```bash
scripts/adoption-evidence.sh /path/to/repo \
  --output-dir /tmp/codeinsight-adoption-evidence \
  --print-snippet \
  --issue-template
```

When you need one uploadable handoff archive:

```bash
scripts/adoption-report.sh /path/to/repo \
  --output-dir /tmp/codeinsight-adoption-report \
  --print-snippet
```

When you only need local first-read evidence artifacts:

```bash
scripts/local-repo-evidence.sh /path/to/repo \
  --output /tmp/codeinsight-local-evidence.md \
  --json /tmp/codeinsight-agent-route.json \
  --summary-json /tmp/codeinsight-local-evidence.json
```

When you need a short comparison for adoption notes:

```bash
scripts/adoption-comparison.sh /path/to/repo \
  --output-dir /tmp/codeinsight-adoption-comparison
```

Pass criteria:

- `indexed_files` is greater than zero.
- `entrypoints` or `recommended_next_tools` is greater than zero.
- `context_pack` reports `selected_files`, `selected_ranges`, and
  `estimated_tokens`.
- The demo prints `first_reading_focus`, `first_reading_question`, executable
  `reading_plan_reason`, `selection_rank`, raw `selection_reason`, and
  `continuation_next_action` for the first selected context file.
- `line_reduction` is present and below 100%.
- `impact_analysis` reports `risk_level`, `impacted_files`, `paths`, or
  `suggested_checks`.
- The final talk track names `agent_route` as the default first-read path and
  includes `project_overview`, `context_pack`, and `impact_analysis` as the
  route internals.
- `local-repo-evidence.sh` writes a Markdown summary with selected lines,
  line reduction, first reading focus/question, first selected file, first
  selection rank/reason, first suggested tool, continuation next action, impact
  risk, and the raw `agent_route` JSON path when `--json` is used.
- `--summary-json` writes the same core metrics in a compact machine-readable
  contract for CI artifacts, README evidence snippets, or benchmark aggregation.
- `adoption-comparison.sh` writes a blind-read vs routed-first-read Markdown
  report with source-line savings, read-less ratio, first reading focus/question,
  first selection rank/reason, and continuation next action. Its `summary.json`
  includes source lines avoided, read-less ratio, seed strategy, first selected
  file, first reading focus/question, and artifact paths.
- Use the [Adoption cases](adoption-cases.md) summary plus the
  [Express adoption case](adoption-case-express.md),
  [Gin adoption case](adoption-case-gin.md),
  [Memchr adoption case](adoption-case-memchr.md), and
  [Requests adoption case](adoption-case-requests.md) as reference shapes for
  public repository comparison snapshots.
- Use the [CodeInsight self adoption report](adoption-report-codeinsight.md) as
  the reference shape for complete `tar.gz` report handoffs that include the
  issue template, manifest, raw MCP first-call JSON, and diagnostic logs.
- Run `scripts/update-self-adoption-report.sh` to refresh that checked-in
  self adoption report from a live `adoption-report` run; use
  `scripts/update-self-adoption-report.sh --check` before release handoff to
  catch stale snapshots.
- Run `scripts/update-adoption-case.sh express` to refresh that checked-in case
  from a live `adoption-comparison` run. The older
  `scripts/update-adoption-case-express.sh` wrapper delegates to the same path.
- Run `scripts/update-adoption-case.sh gin` to refresh the Go adoption case.
- Run `scripts/update-adoption-case.sh memchr` to refresh the Rust library
  adoption case.
- Run `scripts/update-adoption-case.sh requests` to refresh the Python library
  adoption case.
- `adoption-evidence.sh` writes one folder containing local first-read evidence,
  raw `agent_route` JSON, MCP first-call JSON, and aggregate Markdown/JSON
  summaries that prove the CLI route and MCP route both work.
- The aggregate Markdown and JSON summaries list diagnostic stdout/stderr files
  for local evidence generation, MCP first-call verification, and artifact
  writing.
- The aggregate Markdown and JSON summaries include `first_read_gating` signals
  for suggested-tool ordering, continuation ordering, and impact review before
  edits.
- `--print-snippet` prints the same pass/fail evidence shape to stdout so it
  can be copied into an issue, PR, README, or evaluation note without opening
  the artifact files.
- `--issue-template` writes `issue-template.md` with the copyable evidence
  snippet, failure category placeholder, artifact paths, and environment fields
  needed for a reproducible adoption report.
- `adoption-report.sh` writes `codeinsight-adoption-report.tar.gz` with the
  aggregate summaries, issue template, raw route JSON, MCP first-call JSON,
  manifest, and diagnostic stdout/stderr logs.
- Failures are categorized as `[usage]`, `[prerequisite]`,
  `[local_cli_route]`, `[mcp_first_call]`, or `[artifact_write]`, with the
  relevant child-process stderr included for issue reports.

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
- The smoke output includes first reading focus/question evidence, including
  `context_reading_question`,
  `agent_route_reading_question`, `mcp_context_reading_question`,
  `mcp_agent_route_reading_question`, and the matching reading reason and
  selection reason fields.
- The smoke output includes `context_selection_rank`,
  `agent_route_selection_rank`, `mcp_context_selection_rank`,
  `mcp_agent_route_selection_rank`, and the matching `*_read_less_ratio`
  fields.
- The smoke output includes continuation status and next-action fields for
  CLI/MCP `context_pack` and `agent_route`.
- The smoke output includes `agent_route_execution_plan` and
  `mcp_agent_route_execution_plan` with the expected client action order.
- `mcp-stdio-smoke.sh` prints first-reading `selection_rank`,
  `continuation_status`, and first omitted-candidate fields when continuation
  evidence is available.
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
- `agent_route.execution_plan[]` includes `read_selected_context`,
  `use_current_reading_step_suggested_tool`, `use_continuation_if_needed`, and
  `review_impact_before_edits`.
- `agent_route.context_pack.reading_plan[]` is present.
- Client UI or agent policy treats `read_selected_context` as the first active
  step.
- Suggested-tool controls are disabled or visually secondary until the matching
  selected context file has been read.
- Continuation controls are hidden, disabled, or visually secondary until the
  selected context has been consumed and the task still needs more evidence.
- Impact-review controls are shown before edits and are not labeled as a
  safety guarantee.
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
  `agent_route.current_reading_step` for the first checklist row,
  `reading_plan[].focus` as the compact scan label,
  `reading_plan[].question` as the local checklist, and
  `reading_plan[].reason` as the current-step instruction.
- The agent preserves `reading_plan[].selection_rank` as the candidate-rank
  audit trail and can explain `reading_plan[].selection_reason` as the evidence
  for why a file was selected, without treating it as a replacement for
  `reading_plan[].question` or `reading_plan[].reason`.
- The agent does not execute `continuation_summary.suggested_tool` or
  `omitted_candidates[].suggested_tool` until the selected `files[]` excerpts
  have been read.
- The agent uses `continuation_summary.next_action` to describe the post-read
  action instead of inventing a broad follow-up search.
- The agent does not execute `reading_plan[].suggested_tool` until the matching
  selected context file has been read.
- The agent reviews `impact_analysis` before edits without treating it as proof
  that the edit is safe.
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
- `agent_route.current_reading_step` mirrors `reading_plan[0]`.
- `reading_plan[0].focus` is present and provides a compact scan label.
- `reading_plan[0].question` is present and states the local reading
  checklist for the first selected file.
- `reading_plan[0].reason` is present and explains the question, deeper
  evidence tool, and selection rationale.
- `reading_plan[0].selection_rank` is present and greater than zero.
- `reading_plan[0].selection_reason` is present and explains why the first file
  was selected.
- `continuation_summary.status` is present.
- `continuation_summary.next_action` is present.

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
  ranges, `reading_plan_steps`, first selection rank, first next action, token
  budget, and line reduction.

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
- The agent follows `reading_plan[].focus`, `reading_plan[].question`, and
  `reading_plan[].reason`, can surface `reading_plan[].selection_rank` and
  `reading_plan[].selection_reason`, and waits to use continuation tools until
  selected context has been read.
- Suggested-tool and continuation controls are gated behind selected-context
  reading, and impact review is required before edits.
- `impact_analysis` is used before edits.
- Local smoke or benchmark evidence can be reproduced on at least one real
  repository.
