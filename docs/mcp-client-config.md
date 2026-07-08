# MCP Client Configuration

CodeInsight runs as a local stdio MCP server. Most MCP-capable clients need the
same two pieces of information:

- `command`: the `codeinsight` executable path.
- `args`: `["serve", "--transport", "stdio"]`.

## Installed Binary

After installing from source:

```bash
cargo install --path .
```

Use this generic MCP server entry:

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

If the client does not inherit your shell `PATH`, use an absolute binary path:

```json
{
  "mcpServers": {
    "codeinsight": {
      "command": "/absolute/path/to/codeinsight",
      "args": ["serve", "--transport", "stdio"]
    }
  }
}
```

## Local Development Checkout

For a development checkout, run through Cargo:

```json
{
  "mcpServers": {
    "codeinsight-dev": {
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
  }
}
```

## Tool Inputs

CodeInsight tools accept explicit repository or file paths. The server does not
need a global workspace setting.

Example `symbol_search` call arguments:

```json
{
  "root": "/absolute/path/to/repo",
  "query": "AuthService",
  "limit": 20
}
```

Example `context_pack` call arguments:

```json
{
  "root": "/absolute/path/to/repo",
  "task": "understand auth flow",
  "symbols": ["AuthService"],
  "files": ["src/auth.ts"],
  "token_budget": 6000
}
```

Recommended first-read flow for agents:

1. Call `index_project` with `force: false` for the repository.
2. Call `project_overview` and inspect `summary`, `entrypoints`, and
   `main_directories`.
3. Call `context_pack` with only `root`, `task`, and `token_budget` to let
   CodeInsight auto-select the highest-confidence source entrypoint. Provide
   explicit `symbols` or `files` when the user already named a target.

Example `config_status` call arguments:

```json
{
  "root": "/absolute/path/to/repo"
}
```

`config_status` returns whether `.codeinsight/config.toml` exists, whether it
loaded successfully, any `parse_error`, configured impact-analysis commands,
detected fallback test commands, and `commands_override_builtin` so clients can
explain whether configured commands will take precedence over built-in
inference.

`context_pack` returns `files[]` entries with structured `source`, `score`,
`reason`, and `ranges[]` fields. Each range also includes `source`, `score`,
`start_line`, `end_line`, `importance`, `reason`, and `excerpt`, so clients can
sort or filter snippets without parsing explanation text.
It also returns `seed_strategy` and `selected_seeds`; use these fields to show
whether context came from explicit seeds, an overview entrypoint, or indexed
source-file fallback.

Example `context_pack` response shape:

```json
{
  "seed_strategy": "auto_entrypoint",
  "selected_seeds": [
    {
      "kind": "file",
      "value": "src/main.ts",
      "source": "overview_entrypoint",
      "role": "source"
    }
  ],
  "files": [
    {
      "file": "src/auth.ts",
      "source": "call_graph",
      "score": 83,
      "reason": "Selected for medium relevance via call_graph",
      "ranges": [
        {
          "start_line": 12,
          "end_line": 16,
          "source": "call_graph",
          "score": 83,
          "importance": "medium",
          "reason": "Call graph caller of login via AuthController.handle",
          "excerpt": "  12: export function handle(req) {\\n  13:   return login(req);\\n  14: }"
        }
      ]
    }
  ]
}
```

Known `source` values are `seed_file`, `symbol_definition`, `reference`,
`call_graph`, `semantic`, and `dependency`.

It also returns `semantic_status`, including:

- `vector_status`: whether vector matches were available, missing for the
  selected provider/model, or skipped because no provider is configured.
- `vector_candidates` and `fallback_candidates`: semantic candidates generated
  before token-budget selection.
- `selected_vector_ranges` and `selected_fallback_ranges`: semantic ranges that
  actually made it into `files[].ranges[]`.
- `recommendation`: the next action a client can surface, such as running
  `semantic_index` for the selected provider/model.

Run `index_project` first for a repository when you want repeatable results from
the local SQLite index.

For an end-to-end protocol check before configuring a GUI client, see
[MCP client smoke test](mcp-client-smoke.md).
