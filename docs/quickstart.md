# Quickstart

This quickstart takes a new user from install to a working MCP client setup.
It keeps the path local-first: no external database, vector service, or hosted
index is required.

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

The demo runs the product loop:

1. `agent_route`
2. `index_project`
3. `project_overview`
4. `context_pack`
5. `impact_analysis`

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
3. Use continuation_summary only after selected context is consumed.
4. Use focused follow-up tools only when the selected context is insufficient.
5. For custom routing, call index_project, project_overview, context_pack, and
   impact_analysis directly.
```

## 5. Smoke Test MCP

From a development checkout:

```bash
scripts/mcp-stdio-smoke.sh
```

Against a real repository:

```bash
CODEINSIGHT_SMOKE_ROOT=/path/to/repo scripts/mcp-stdio-smoke.sh
```

With an installed binary:

```bash
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/mcp-stdio-smoke.sh
```

To verify the installed binary without using this repository as the target
project:

```bash
scripts/installed-quickstart-smoke.sh
```

The MCP stdio smoke output starts with:

```text
MCP stdio smoke passed
tools: 16
```

The installed quickstart smoke prints `installed quickstart smoke passed` after
the installed binary completes `version`, `index`, `overview`, `context-pack`,
`agent-route`, and MCP stdio calls against a temporary project. The MCP portion
also calls `agent_route` so installed clients exercise the default first-read
route.

## 6. First Agent Task

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
