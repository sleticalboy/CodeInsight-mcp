# MCP Client Smoke Test

This document verifies the stdio MCP path before wiring CodeInsight into an MCP
client.

## Server Command

Installed binary:

```json
{
  "command": "codeinsight",
  "args": ["serve", "--transport", "stdio"]
}
```

Development checkout:

```json
{
  "command": "cargo",
  "args": [
    "run",
    "--manifest-path",
    "/absolute/path/to/CodeInsight-mcp/Cargo.toml",
    "--",
    "serve",
    "--transport",
    "stdio"
  ]
}
```

## Local Smoke

Run the protocol smoke test from the repository root:

```bash
scripts/mcp-stdio-smoke.sh
```

The script starts `codeinsight serve --transport stdio` and verifies this
sequence:

1. `initialize`
2. `tools/list`
3. `tools/call` for `index_project`
4. `tools/call` for `project_overview`
5. `tools/call` for `symbol_search`
6. `tools/call` for `embedding_status`
7. `tools/call` for `agent_route`
8. `tools/call` for explicit-seed `context_pack`
9. `tools/call` for auto-entrypoint `context_pack`

It also asserts the MCP-facing structured fields that clients commonly render:

- `project_overview.recommended_next_tools`
- `project_overview.recommended_next_tools` calls for `context_pack` and
  `config_status` execute
- `agent_route.route[]` includes `index_project`, `project_overview`,
  `context_pack`, and `impact_analysis`
- `agent_route.execution_plan[]` includes `read_selected_context`,
  `use_current_reading_step_suggested_tool`, `use_continuation_if_needed`, and
  `review_impact_before_edits`
- first `agent_route.execution_plan[]` step is ready and names selected files
- second `agent_route.execution_plan[]` step exposes the current-step
  `suggested_tool`
- second `agent_route.execution_plan[]` suggested tool executes through
  MCP `tools/call`
- explicit and auto `context_pack.reading_plan`
- `context_pack.reading_plan[].next_action` and `question`
- `context_pack.reading_plan[].suggested_tool`
- first explicit and auto `reading_plan[].suggested_tool` calls execute
- explicit `context_pack.budget` metadata matches legacy top-level fields
- explicit `context_pack.continuation_summary` exposes a client-facing next
  action
- explicit `context_pack.omitted_candidates` is present, excerpt-free, and its
  first suggested follow-up executes when omitted files exist

Use a real repository instead of the generated fixture:

```bash
CODEINSIGHT_SMOKE_ROOT=/absolute/path/to/repo scripts/mcp-stdio-smoke.sh
```

Choose the seed symbol for `symbol_search` and `context_pack`:

```bash
CODEINSIGHT_SMOKE_ROOT=/absolute/path/to/repo \
CODEINSIGHT_SMOKE_SYMBOL=Store \
scripts/mcp-stdio-smoke.sh
```

Use an installed binary instead of the local release build:

```bash
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/mcp-stdio-smoke.sh
```

## Expected Output

```text
MCP stdio smoke passed
root: /path/to/repo
symbol: AuthService
tools: 16
indexed_files: 33
overview_entrypoints: 1
overview_recommendations: 4
overview_context_seed_strategy: auto_entrypoint
auto_seed_strategy: auto_entrypoint
auto_reading_plan_steps: 2
agent_route_execution_plan_steps: 4
agent_route_first_execution_action: read_selected_context
agent_route_suggested_tool: file_outline
agent_route_suggested_tool_executed: true
explicit_suggested_tool: file_outline
auto_suggested_tool: file_outline
explicit_omitted_candidates: 8
```

`indexed_files`, `overview_entrypoints`, `overview_recommendations`, and
`auto_reading_plan_steps` vary with the tested repository. Suggested tool names
and `explicit_omitted_candidates` also vary with the selected first reading
step, seed symbol, and token budget.
`agent_route_execution_plan_steps` should remain `4` for the default first-read
route unless the contract changes deliberately.
`agent_route_suggested_tool_executed` should remain `true`; it verifies the
execution-plan suggested tool is a usable MCP call, not only display metadata.

## Troubleshooting

- `missing required command: python3`: install Python 3; the smoke script uses
  the standard library to validate JSON-RPC responses.
- `No such file or directory`: use an absolute path for `CODEINSIGHT_BIN` or
  install the binary into a directory available to the client process.
- Empty search results: run `index_project` first, then call `symbol_search`.
  For the smoke script, set `CODEINSIGHT_SMOKE_SYMBOL` to a symbol that exists
  in the tested repository.
- Permission errors in clients: prefer an absolute `command` path because GUI
  clients often do not inherit shell `PATH`.
- Stale index behavior: pass `force: true` to `index_project` when validating a
  fresh checkout.
