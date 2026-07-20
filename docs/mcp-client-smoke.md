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
- `project_overview.dependency_summary.type_relation_edges` and
  `top_type_relation_targets`
- `project_overview.recommended_next_tools` calls for `context_pack` and
  `config_status` execute, and the type-relation `dependency_graph`
  recommendation executes
- `agent_route.route[]` includes `index_project`, `project_overview`,
  `context_pack`, and `impact_analysis`
- `agent_route.execution_plan[]` includes `read_selected_context`,
  `use_current_reading_step_suggested_tool`, `use_continuation_if_needed`, and
  `review_impact_before_edits`
- `agent_route.current_reading_step` mirrors
  `agent_route.context_pack.reading_plan[0]`
- first `agent_route.execution_plan[]` step is ready and names selected files
- second `agent_route.execution_plan[]` step exposes the current-step
  `suggested_tool`
- second `agent_route.execution_plan[]` suggested tool executes through
  MCP `tools/call`
- suggested-tool execution is proof the follow-up call is usable, not permission
  to run it before selected context is read
- continuation follow-ups remain gated behind selected-context reading even
  when omitted candidates expose ready MCP calls
- `review_impact_before_edits` remains the pre-edit planning checkpoint, not a
  safety guarantee
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
agent_route_current_reading_step_matches_reading_plan: true
agent_route_source_lines_avoided: 156
agent_route_read_less_ratio: 20.5x
agent_route_first_reading_selection_rank: 1
agent_route_continuation_status: complete
agent_route_continuation_next_action: read_selected_context
agent_route_first_omitted_file: -
agent_route_first_omitted_selection_rank: -
agent_route_first_omitted_omission_reason: -
agent_route_suggested_tool: file_outline
agent_route_suggested_tool_executed: true
explicit_first_reading_selection_rank: 1
explicit_source_lines_avoided: 146
explicit_read_less_ratio: 9.1x
explicit_continuation_status: omitted_candidates_available
explicit_continuation_next_action: run_omitted_candidate_context_pack
explicit_first_omitted_file: src/consumer_14.py
explicit_first_omitted_selection_rank: 7
explicit_first_omitted_omission_reason: token_budget_exhausted
explicit_suggested_tool: file_outline
auto_source_lines_avoided: 156
auto_read_less_ratio: 20.5x
auto_suggested_tool: file_outline
explicit_omitted_candidates: 8
```

`indexed_files`, `overview_entrypoints`, `overview_recommendations`, and
`auto_reading_plan_steps` vary with the tested repository. Suggested tool names
and `explicit_omitted_candidates` also vary with the selected first reading
step, seed symbol, and token budget.
`agent_route_execution_plan_steps` should remain `4` for the default first-read
route unless the contract changes deliberately.
`agent_route_current_reading_step_matches_reading_plan` should remain `true`;
it verifies the protocol-level shortcut mirrors the first reading-plan row.
`agent_route_source_lines_avoided` and `agent_route_read_less_ratio` should
remain present so MCP client smoke output exposes the same source-line
compression evidence as the compact first-call summary.
`explicit_read_less_ratio` and `auto_read_less_ratio` should also remain
present because the protocol smoke exercises direct `context_pack` calls as
well as `agent_route`.
`agent_route_suggested_tool_executed` should remain `true`; it verifies the
execution-plan suggested tool is a usable MCP call, not only display metadata.
`agent_route_first_reading_selection_rank` and
`explicit_first_omitted_omission_reason` should remain present so protocol
smoke output exposes the same candidate-ranking and continuation evidence as
the compact MCP first-call summary.
Clients should still gate that action behind selected-context reading. The
smoke proves protocol usability; it does not change the first-read ordering
contract.

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
