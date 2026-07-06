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
4. `tools/call` for `symbol_search`
5. `tools/call` for `context_pack`

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
tools: 10
indexed_files: 3
```

`indexed_files` varies with the tested repository.

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
