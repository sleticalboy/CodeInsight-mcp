# MCP Tools

CodeInsight exposes its local code-intelligence functions through MCP stdio.
Start the server with:

```bash
codeinsight serve --transport stdio
```

For client command snippets, see [MCP client configuration](mcp-client-config.md).
For protocol smoke testing, see [MCP client smoke test](mcp-client-smoke.md).

## Protocol

The server supports:

- `initialize`
- `tools/list`
- `tools/call`

Example `tools/call` request:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"symbol_search","arguments":{"root":"/path/to/repo","query":"AuthService","limit":5}}}
```

Tool responses include `structuredContent` so clients can render and route
results without parsing text.

## Tool List

The stdio server currently exposes 16 tools:

| Tool | Purpose |
| --- | --- |
| `index_project` | Index a local repository for repeatable local analysis, including the applied `.codeinsight/config.toml` index scope in the response. |
| `config_status` | Report `.codeinsight/config.toml`, load status, parse errors, configured index scope, configured JavaScript package conditions, configured impact-analysis checks, detected fallback test commands, and whether configured commands override built-in inference. |
| `project_overview` | Return the repository briefing an agent should fetch first: summaries, role-aware directories, entrypoint candidates, dependency/type-relation summaries, `recommended_next_tools`, and index metadata. |
| `symbol_search` | Search extracted symbols in an indexed repository. |
| `file_outline` | Parse one source file and return a symbol outline. |
| `dependency_graph` | Return module-level dependencies extracted during indexing, including type-relation edge counts and top relation targets, optionally filtered by touching files, languages, or dependency `kinds` and paged with `limit` / `offset`. |
| `impact_analysis` | Estimate local impact radius from seed symbols or files using definitions, text references, static callers, local callee targets, and resolved dependencies; returns ranked files, paths, risk, reasons, and suggested checks. |
| `find_references` | Find ranked text references across indexed files with file, location, context, approximate reference kind, and confidence. |
| `semantic_search` | Query local semantic vectors through a configured embedding provider. |
| `semantic_index` | Build local semantic text chunks and optional embeddings; can report incremental chunk changes. |
| `embedding_status` | Report provider, batch size, and optional local semantic-index state without network calls. |
| `version` | Return package version and target platform information. |
| `context_pack` | Build token-budgeted agent context from explicit seeds or inferred entrypoints, including selected files/ranges, source mix counts, seed strategy, selected seeds, read-less source-line metrics, budget metadata, continuation summary, omitted candidate follow-ups, reading plan, semantic status, and follow-up suggestions. |
| `agent_route` | Run the default first-read path in one call: refresh the local index, return `project_overview`, build `context_pack`, expose `current_reading_step` and `execution_plan[]`, include an `impact_analysis` preview when a seed is available, and return a structured blocked plan when no source seed can be inferred. |
| `callers` | Return static call sites that call a function or method, including imported target hints when available. |
| `callees` | Return static callees for a function or method, including imported target hints when available. |

## Recommended First Read

Recommended MCP first-read flow:

1. Call `agent_route` with `root`, `task`, and `token_budget` for the default
   first-read path.
2. Follow `agent_route.execution_plan[]`: read selected context, use the
   current-step `suggested_tool` only when needed, continue only after selected
   context, and review impact before edits.
3. Use `agent_route.current_reading_step` to render the first checklist row
   without rebuilding it from `context_pack.reading_plan[0]`.
   If it is omitted, inspect `execution_plan[]` statuses such as
   `blocked_no_reading_plan` or `blocked_no_current_reading_step` and ask for a
   seed file or symbol instead of broad-reading the repository.
4. Display `context_pack.read_less` when users need to see how much source
   text the first read avoided before follow-up tools.
5. Use the lower-level tools directly when the client needs step-by-step
   control: `index_project`, `project_overview`, `context_pack`, then
   `impact_analysis`.

For the full response contract, see [First-read workflow](first-read-workflow.md).
For an end-to-end client consumption flow, see
[Client workflow](client-workflow.md), including a copyable agent policy prompt
and task-routing matrix.
For recommendation priorities and client sorting guidance, see
[Recommendation contract](recommendation-contract.md).

## Topic Contracts

- Overview and context-pack ranking: [First-read workflow](first-read-workflow.md)
- End-to-end client flow: [Client workflow](client-workflow.md)
- `recommended_next_tools` and `reading_plan[].suggested_tool`:
  [Recommendation contract](recommendation-contract.md)
- `agent_route.execution_plan[]`:
  [Recommendation contract](recommendation-contract.md#agent-route-execution-plan)
- `find_references`, `callers`, and `callees`:
  [Navigation tools](navigation-tools.md)
- `impact_analysis` and `config_status`:
  [Impact analysis](impact-analysis.md)
- `semantic_search`, `semantic_index`, and `embedding_status`:
  [Embedding providers](embedding-providers.md)
- Accuracy boundaries and non-goals:
  [Known limitations](known-limitations.md)
