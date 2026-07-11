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

The stdio server currently exposes 15 tools:

| Tool | Purpose |
| --- | --- |
| `index_project` | Index a local repository for repeatable local analysis. |
| `config_status` | Report `.codeinsight/config.toml`, load status, parse errors, configured JavaScript package conditions, configured impact-analysis checks, detected fallback test commands, and whether configured commands override built-in inference. |
| `project_overview` | Return the repository briefing an agent should fetch first: summaries, role-aware directories, entrypoint candidates, `recommended_next_tools`, and index metadata. |
| `symbol_search` | Search extracted symbols in an indexed repository. |
| `file_outline` | Parse one source file and return a symbol outline. |
| `dependency_graph` | Return module-level dependencies extracted during indexing, optionally filtered by touching files or languages and paged with `limit` / `offset`. |
| `impact_analysis` | Estimate local impact radius from seed symbols or files using definitions, text references, static calls, and resolved local dependencies; returns ranked files, paths, risk, reasons, and suggested checks. |
| `find_references` | Find ranked text references across indexed files with file, location, context, approximate reference kind, and confidence. |
| `semantic_search` | Query local semantic vectors through a configured embedding provider. |
| `semantic_index` | Build local semantic text chunks and optional embeddings; can report incremental chunk changes. |
| `embedding_status` | Report provider, batch size, and optional local semantic-index state without network calls. |
| `version` | Return package version and target platform information. |
| `context_pack` | Build token-budgeted agent context from explicit seeds or inferred entrypoints, including selected files/ranges, seed strategy, selected seeds, budget metadata, reading plan, semantic status, and follow-up suggestions. |
| `callers` | Return static call sites that call a function or method, including imported target hints when available. |
| `callees` | Return static callees for a function or method, including imported target hints when available. |

## Recommended First Read

Recommended MCP first-read flow:

1. Call `index_project` for the repository.
2. Call `project_overview` to inspect summary, roles, entrypoint candidates,
   and `recommended_next_tools`.
3. Call `context_pack` with `root`, `task`, and `token_budget`. Omit `symbols`
   and `files` to let CodeInsight auto-select the highest-confidence source
   entrypoint.

For the full response contract, see [First-read workflow](first-read-workflow.md).
For recommendation priorities and client sorting guidance, see
[Recommendation contract](recommendation-contract.md).

## Topic Contracts

- Overview and context-pack ranking: [First-read workflow](first-read-workflow.md)
- `recommended_next_tools` and `reading_plan[].suggested_tool`:
  [Recommendation contract](recommendation-contract.md)
- `find_references`, `callers`, and `callees`:
  [Navigation tools](navigation-tools.md)
- `impact_analysis` and `config_status`:
  [Impact analysis](impact-analysis.md)
- `semantic_search`, `semantic_index`, and `embedding_status`:
  [Embedding providers](embedding-providers.md)
- Accuracy boundaries and non-goals:
  [Known limitations](known-limitations.md)
