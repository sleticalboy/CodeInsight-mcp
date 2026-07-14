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
scripts/agent-router-demo.sh
```

Against another repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/agent-router-demo.sh
```

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
scripts/agent-router-demo.sh
```

Point out the four stages:

1. `index_project`
2. `project_overview`
3. `context_pack`
4. `impact_analysis`

Expected shape:

```text
1. index_project
   indexed_files: 23
   symbols: 911

2. project_overview
   entrypoints: 7
   recommended_next_tools: 4

3. context_pack
   selected_files: 10
   selected_ranges: 11
   reading_plan_steps: 10
   first_next_action: inspect_seed_file
   line_reduction: 98.4%
   continuation: complete

4. impact_analysis
   risk_level: high
   impacted_files: 11
   suggested_checks: 3
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

### 1:35-1:55 - MCP Agent Flow

In a real MCP client, the agent policy is:

```text
1. Call index_project.
2. Call project_overview.
3. Call context_pack with root, task, and token_budget.
4. Read context_pack.files in reading_plan order.
5. Use continuation_summary only after selected context is consumed.
6. Call impact_analysis before edits.
```

This is the path that turns CodeInsight from a CLI demo into an agent workflow.

### 1:55-2:00 - Close

The product promise is simple: keep code local, route the agent to the right
context, and reduce blind reading before edits.

## Demo Checklist

Before recording or presenting:

- Run `cargo build --locked --release`.
- Run `scripts/agent-router-demo.sh`.
- Confirm `indexed_files` is greater than zero.
- Confirm `recommended_next_tools` is greater than zero.
- Confirm `context_pack` reports selected files, `reading_plan_steps`, and
  `line_reduction`; the demo fails fast if selected files, reading-plan steps,
  or the first next action are missing.
- Confirm `impact_analysis` reports risk or suggested checks.
- Keep [Known limitations](known-limitations.md) available if asked about
  static-analysis precision.

## If The Demo Is Run On Another Repository

Use:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo \
CODEINSIGHT_DEMO_TASK="understand the main application entrypoint" \
scripts/agent-router-demo.sh
```

If the repository has no obvious source entrypoint, explain that as product
signal: `project_overview` still recommends a fallback tool path, often
`context_pack`, `dependency_graph`, `callers`, or `config_status`.

## Suggested Follow-Up Links

- [Quickstart](quickstart.md)
- [Adoption checklist](adoption-checklist.md)
- [Client workflow](client-workflow.md)
- [MCP client configuration](mcp-client-config.md)
- [Benchmark reports](benchmark-v0.1.md)
