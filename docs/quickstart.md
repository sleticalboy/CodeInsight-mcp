# Quickstart

This quickstart takes a new user from install to a working MCP client setup.
It keeps the path local-first: no external database, vector service, or hosted
index is required.

The main path is intentionally one call from the agent: `agent_route`. It
refreshes the local index, returns a repository overview, selects the bounded
context pack, and includes an impact preview for edit planning.

## Fast Path

1. Install `codeinsight`.
2. Configure the local stdio MCP server:
   `codeinsight serve --transport stdio`.
3. Add the agent policy so broad repository tasks start with `agent_route`.
4. Run `scripts/two-minute-demo.sh` for a visible evidence summary, or
   `scripts/mcp-first-call-smoke.sh` for a copyable MCP first-call JSON
   summary, or
   `scripts/installed-quickstart-smoke.sh` for the installed-binary adoption
   gate.

## 1. Install

Install the latest macOS or Linux release:

```bash
curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | sh
```

Or install with Homebrew:

```bash
brew tap sleticalboy/tap
brew install codeinsight
```

For a development checkout:

```bash
cargo install --path .
```

Verify the binary:

```bash
codeinsight version
```

If your MCP client does not inherit shell `PATH`, use the absolute path from:

```bash
command -v codeinsight
```

## 2. Run The Local Demo

From the repository root:

```bash
scripts/two-minute-demo.sh
```

Against another repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

The demo calls `agent_route`. The returned `route[]` records the local work
CodeInsight already performed:

1. `index_project`
2. `project_overview`
3. `context_pack`
4. `impact_analysis`

It prints index timing, entrypoint count, recommended-tool count, selected
context size, line reduction, continuation status, impact summary, and a short
talk track that explains why each step matters.

## 3. Configure Your MCP Client

Use the installed binary:

```json
{
  "mcpServers": {
    "codeinsight": {
      "command": "codeinsight",
      "args": ["serve", "--transport", "stdio"]
    }
  }
}
```

For clients that require `type`:

```json
{
  "mcpServers": {
    "codeinsight": {
      "type": "stdio",
      "command": "codeinsight",
      "args": ["serve", "--transport", "stdio"]
    }
  }
}
```

Codex users can add this to `~/.codex/config.toml`:

```toml
[mcp_servers.codeinsight]
type = "stdio"
command = "codeinsight"
args = ["serve", "--transport", "stdio"]
startup_timeout_sec = 30
tool_timeout_sec = 120
```

See [MCP client configuration](mcp-client-config.md) for Codex, Claude Code,
Cursor, and generic MCP JSON examples.

## 4. Add The Agent Policy

Add the policy from [Client workflow](client-workflow.md#agent-policy-prompt)
to your client's project instructions:

- Codex: repo-level `AGENTS.md`
- Claude Code: project instructions or session prompt
- Cursor: project rules or agent prompt

Minimum policy:

```text
Before broad repository reading, use CodeInsight:
1. Call agent_route with root, task, and token_budget for the default first read.
2. Read context_pack.files in reading_plan order.
3. Use reading_plan.focus as the compact scan label, reading_plan.question as
   the local checklist, and reading_plan.reason as the current-step
   instruction.
4. Use continuation_summary only after selected context is consumed.
5. Use focused follow-up tools only when the selected context is insufficient.
6. For custom routing, call index_project, project_overview, context_pack, and
   impact_analysis directly.
```

## 5. Choose A Smoke Check

Use the narrowest check that matches where you are in adoption:

| Situation | Command | What It Proves |
| --- | --- | --- |
| You want a visible product walkthrough | `scripts/two-minute-demo.sh` | `agent_route` selects bounded context, prints `[Evidence summary]`, and frames the pre-edit impact check. |
| You want framework entrypoint evidence | `scripts/framework-entrypoint-demo.sh` | Next.js, Rails, Django, and C# web entrypoints are detected by `project_overview` and selected first by matching `context_pack` tasks. |
| You want a multi-task route quality check | `scripts/task-routing-matrix.sh /path/to/repo --expect-file ./route-expectations.tsv` | Runs routing/auth/authorization/access-control/settings/feature flag/network/TLS/validation/startup/persistence/debug/coverage/API handler/cache/observability/security/billing/frontend/background job/documentation/request lifecycle/middleware prompts and writes a Markdown/JSON matrix with first selected file, seed strategy, line reduction, token estimate, impact preview, and optional expected-file gates. |
| You want a copyable first MCP call summary | `scripts/mcp-first-call-smoke.sh` | The stdio server accepts `agent_route` and returns the first context file, `reading_plan[]`, execution plan contract checks, current-step instruction checks, `suggested_tool_executed`, and `impact_status` as JSON. |
| You are wiring an MCP client from this checkout | `scripts/mcp-stdio-smoke.sh` | The stdio server lists tools, runs `agent_route`, executes `agent_route.execution_plan[].suggested_tool`, and prints read-less, selection, and continuation evidence through MCP. |
| You installed `codeinsight` and want an adoption gate | `CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh` | The installed binary can run CLI and MCP first-read routes with read-less, selection-rank, and continuation evidence against a temporary project outside this checkout. |
| You need adoption comparison evidence | `scripts/adoption-comparison.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-comparison` | A blind-read vs routed-first-read report with source lines avoided, read-less ratio, seed strategy, first reading focus/question, selection rank, and continuation next action. |
| You want evidence for your own repository | `CODEINSIGHT_BENCH_PROFILE=local CODEINSIGHT_BENCH_LOCAL_ROOT=/path/to/repo CODEINSIGHT_BENCH_LOCAL_CONTEXT_FILE=src/main.ts CODEINSIGHT_BENCH_OUTPUT=/tmp/codeinsight-local-benchmark.md scripts/benchmark-smoke.sh` | A shareable benchmark report with routing, compression, reading-plan, and guardrail evidence for one local checkout. |

## 6. Smoke Test MCP

From a development checkout:

```bash
scripts/mcp-first-call-smoke.sh
scripts/mcp-stdio-smoke.sh
```

`mcp-first-call-smoke.sh` prints a compact JSON summary for the first MCP
`agent_route` call. Use it when you want to confirm the server, route, selected
files, read-less metrics, selection rank, continuation summary, reading plan
order, suggested tool handoff, and impact preview without reading the full
protocol smoke log.

Run `scripts/mcp-first-call-smoke.sh --help` to see the supported environment
variables for binary path, target repository, task, and token budget.
Use `scripts/mcp-first-call-smoke.sh --summary-json /tmp/codeinsight-mcp-first-call.json`
when you want to keep the summary as an artifact while still printing it to
stdout.

Expected output shape:

```json
{
  "status": "pass",
  "server": "codeinsight",
  "route_tools": [
    "index_project",
    "project_overview",
    "context_pack",
    "impact_analysis"
  ],
  "selected_files": ["src/main.ts", "src/auth.ts"],
  "first_context_file": "src/main.ts",
  "first_reading_file": "src/main.ts",
  "first_reading_selection_rank": 1,
  "current_reading_step_matches_reading_plan": true,
  "context_pack_read_less": {
    "baseline_source_lines": 18,
    "selected_source_lines": 15,
    "source_lines_avoided": 3,
    "line_reduction": "16.7%",
    "read_less_ratio": "1.2x"
  },
  "baseline_source_lines": 18,
  "selected_source_lines": 15,
  "source_lines_avoided": 3,
  "line_reduction": "16.7%",
  "read_less_ratio": "1.2x",
  "reading_plan": [
    {
      "file": "src/main.ts",
      "selection_rank": 1,
      "next_action": "inspect_seed_file",
      "focus": "Start with seed file context and primary symbols.",
      "question": "What entrypoints, exported symbols, or setup code define the main flow here?",
      "reason": "Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file",
      "selection_reason": "Selected for high relevance via seed_file",
      "suggested_tool": "file_outline"
    }
  ],
  "execution_plan_actions": [
    "read_selected_context",
    "use_current_reading_step_suggested_tool",
    "use_continuation_if_needed",
    "review_impact_before_edits"
  ],
  "execution_plan_reads_in_reading_plan_order": true,
  "first_execution_instruction_has_focus": true,
  "first_execution_instruction_has_question": true,
  "current_step_suggested_tool_matches_reading_plan": true,
  "current_step_instruction_has_focus": true,
  "current_step_instruction_has_question": true,
  "current_step_instruction_has_action": true,
  "continuation_after_selected_context": true,
  "continuation_status": "complete",
  "continuation_next_action": "read_selected_context",
  "first_omitted_file": "",
  "first_omitted_selection_rank": null,
  "first_omitted_omission_reason": "",
  "first_omitted_next_action": "",
  "suggested_tool": {
    "tool": "file_outline",
    "arguments": {
      "path": "/absolute/path/to/repo/src/main.ts"
    }
  },
  "suggested_tool_executed": true,
  "impact_status": "complete"
}
```

Against a real repository:

```bash
CODEINSIGHT_FIRST_CALL_ROOT=/path/to/repo scripts/mcp-first-call-smoke.sh
CODEINSIGHT_SMOKE_ROOT=/path/to/repo scripts/mcp-stdio-smoke.sh
```

With an installed binary:

```bash
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/mcp-first-call-smoke.sh
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/mcp-stdio-smoke.sh
```

To verify the installed binary without using this repository as the target
project:

```bash
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh
```

The MCP stdio smoke output starts with:

```text
MCP stdio smoke passed
tools: 16
```

The installed quickstart smoke prints `installed quickstart smoke passed` after
the installed binary completes `version`, `index`, `overview`, `context-pack`,
CLI `agent-route`, MCP stdio, and MCP `agent_route` calls against a temporary
project. It also checks `agent_route.execution_plan[]`,
`reading_plan.focus`, `reading_plan.question`, `reading_plan.reason`,
`selection_reason`, `selection_rank`, and continuation evidence in both CLI and
MCP first-read paths. This is the same installed-binary adoption gate referenced
by the [Adoption checklist](adoption-checklist.md).

## 7. First Agent Task

Ask your MCP-enabled agent:

```text
Use CodeInsight to understand this repository before reading files directly.
Start with agent_route for:
"understand the main application entrypoint"
Use a token budget of 6000.
```

Before making a code change, ask:

```text
Use CodeInsight impact_analysis on the files or symbols you plan to edit.
Report risk_level, impacted_files, paths, and suggested_checks before changing code.
```

## Troubleshooting

- MCP server does not start: use an absolute `command` path.
- Search returns nothing: run `index_project` first.
- Context is too broad: pass a narrower `task`, `files`, or `symbols`.
- Context is truncated: read selected context first, then run
  `continuation_summary.suggested_tool` when present.
- Client config differs from these examples: check
  [MCP client configuration](mcp-client-config.md) and the official client docs.

## Next

Use the [Adoption checklist](adoption-checklist.md) to verify that CodeInsight
is fully wired into your MCP client and agent workflow.
