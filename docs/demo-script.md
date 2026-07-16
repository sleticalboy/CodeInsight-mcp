# Demo Script

Use this script for a two-minute walkthrough, README video, conference demo, or
open-source project introduction. The goal is to show CodeInsight as a
local-first context router for AI coding agents, not as a replacement for an
IDE, LSP, compiler, or hosted code search platform.

## Demo Goal

Show that an agent can start from a repository root and quickly get:

- a local index
- likely entrypoints
- a token-budgeted context pack
- next recommended tools
- an impact-analysis preview before edits

## Setup

From a CodeInsight checkout:

```bash
cargo build --locked --release
scripts/two-minute-demo.sh
```

Against another repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

For machine-readable CI validation, `scripts/two-minute-demo.sh` runs the
`agent-route` CLI command and renders the first-read metrics. Use
`scripts/agent-router-demo.sh` when you only need the lower-level raw metrics
and CI-style assertions.

For a copyable example of the current repository output, see the
[demo output snapshot](demo-output.md).

Optional smoke validation:

```bash
scripts/mcp-stdio-smoke.sh
```

## Two-Minute Talk Track

### 0:00-0:20 - Problem

AI coding agents waste time and tokens on the first read of a repository. They
often start by guessing search terms, running broad file scans, opening too
many files, and still missing the actual entrypoint or change radius.

The core problem is not editing code. The core problem is routing the agent to
the right local context before it edits.

### 0:20-0:35 - Positioning

CodeInsight is a local-first MCP code context router. It indexes the repository
locally, summarizes structure, chooses a bounded context pack, and suggests the
next tool call.

It is intentionally not trying to be a compiler-grade analyzer or a replacement
for Sourcegraph. It is designed to make an agent read less code and make fewer
blind guesses.

### 0:35-1:10 - Run The Demo

Run:

```bash
scripts/two-minute-demo.sh
```

Point out the four stages:

1. `index_project`
2. `project_overview`
3. `context_pack`
4. `impact_analysis`

Expected shape:

```text
CodeInsight two-minute demo

Problem: AI agents waste the first read by scanning broad files and guessing entrypoints.
Promise: route the agent through agent_route before edits.

1. index_project
   indexed_files: 23
   symbols: 926

2. project_overview
   entrypoints: 7
   recommended_next_tools: 4

3. context_pack
   selected_files: 2
   selected_ranges: 4
   reading_plan_steps: 2
   first_next_action: inspect_seed_file
   line_reduction: 99.6%
   continuation: complete

4. impact_analysis
   risk_level: high
   impacted_files: 7
   suggested_checks: 4

[Talk track]
1. agent_route ran index_project, project_overview, context_pack, and impact_analysis in one call.
2. project_overview found 7 entrypoints and 4 recommended next tools.
3. context_pack selected 2 files and 4 ranges, then produced 2 reading-plan steps.
4. The first action is inspect_seed_file; the selected context reduced source reading by 99.6%.
```

Exact numbers vary by repository and current source state. The important point
is that CodeInsight turns a repository into a small, ordered reading plan.

### 1:10-1:35 - Explain The Output

`project_overview` answers "where should the agent start?" It returns
entrypoint candidates, directory roles, summaries, and recommended next tools.

`context_pack` answers "what should fit into the model context now?" It returns
selected files, line ranges, excerpts, a reading plan, budget metadata, and a
continuation strategy if more context is needed.

`line_reduction` shows the routing value. The agent does not need to read the
whole repository to start. It can begin with a bounded context pack and ask for
focused continuation only when necessary.

`impact_analysis` answers "what might I affect if I change this?" It gives a
local risk preview and suggested checks before edits.

### 1:35-1:50 - Evidence Cutaway

For a README video or project introduction, briefly point to the benchmark
reports after the live demo:

```text
Smoke benchmark: context_pack first for 4/4 repositories, 99.2% aggregate line reduction.
Large benchmark: context_pack first for 4/4 repositories, 99.3% aggregate line reduction.
```

The `Key Results` section in each report is the stable evidence slide. It
summarizes routing, context compression, token budget, indexing, guardrails,
and truncation status without asking viewers to inspect the full details table.

### 1:50-1:58 - MCP Agent Flow

In a real MCP client, the agent policy is:

```text
1. Call agent_route with root, task, and token_budget for the default first read.
2. Read context_pack.files in reading_plan order.
3. Use continuation_summary only after selected context is consumed.
4. Run focused follow-up tools only when needed.
```

This is the path that turns CodeInsight from a CLI demo into an agent workflow.
Clients that need custom routing can still call `index_project`,
`project_overview`, `context_pack`, and `impact_analysis` directly.

### 1:58-2:00 - Close

The product promise is simple: keep code local, route the agent to the right
context, and reduce blind reading before edits.

## Demo Checklist

Before recording or presenting:

- Run `cargo build --locked --release`.
- Run `scripts/two-minute-demo.sh`.
- Run `scripts/agent-router-demo.sh` when you need the raw assertion-oriented
  metrics.
- Compare with [Demo output snapshot](demo-output.md) when preparing README
  videos, release notes, or project introductions.
- Confirm `indexed_files` is greater than zero.
- Confirm `recommended_next_tools` is greater than zero.
- Confirm `context_pack` reports selected files, `reading_plan_steps`, and
  `line_reduction`; the demo fails fast if selected files, reading-plan steps,
  or the first next action are missing.
- Confirm `impact_analysis` reports risk or suggested checks.
- Keep the benchmark `Key Results` available if the audience asks for evidence
  beyond the live demo.
- Keep [Known limitations](known-limitations.md) available if asked about
  static-analysis precision.

## If The Demo Is Run On Another Repository

Use:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo \
CODEINSIGHT_DEMO_TASK="understand the main application entrypoint" \
scripts/two-minute-demo.sh
```

If the repository has no obvious source entrypoint, explain that as product
signal: `project_overview` still recommends a fallback tool path, often
`context_pack`, `dependency_graph`, `callers`, or `config_status`.

## Suggested Follow-Up Links

- [Quickstart](quickstart.md)
- [Adoption checklist](adoption-checklist.md)
- [Client workflow](client-workflow.md)
- [MCP client configuration](mcp-client-config.md)
- [Smoke benchmark](benchmark-v0.1.md)
- [Large repository benchmark](benchmark-large.md)
