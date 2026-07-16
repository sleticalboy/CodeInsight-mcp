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

## Client-Specific Setup

Use an installed `codeinsight` binary when possible. GUI clients often do not
inherit your shell `PATH`, so use an absolute `command` path if the server does
not start.

### Codex

Add this to `~/.codex/config.toml`:

```toml
[mcp_servers.codeinsight]
type = "stdio"
command = "codeinsight"
args = ["serve", "--transport", "stdio"]
startup_timeout_sec = 30
tool_timeout_sec = 120
```

If Codex cannot find the binary:

```toml
[mcp_servers.codeinsight]
type = "stdio"
command = "/absolute/path/to/codeinsight"
args = ["serve", "--transport", "stdio"]
startup_timeout_sec = 30
tool_timeout_sec = 120
```

Put the [Agent Policy Prompt](client-workflow.md#agent-policy-prompt) in a
repo-level `AGENTS.md` when you want Codex to consistently use CodeInsight for
first-read routing in that repository.

### Claude Code

For a personal local server in the current project:

```bash
claude mcp add --transport stdio codeinsight -- codeinsight serve --transport stdio
```

For a project-shared `.mcp.json` entry:

```bash
claude mcp add --transport stdio --scope project codeinsight -- codeinsight serve --transport stdio
```

The resulting project file can also be written directly:

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

Claude Code prompts before using project-scoped `.mcp.json` servers in a newly
trusted checkout. Keep the [Agent Policy Prompt](client-workflow.md#agent-policy-prompt)
in project instructions or paste it into the session when you want the agent to
prefer CodeInsight before ad hoc repository search.

### Cursor

For a user-level Cursor MCP entry, add this to `~/.cursor/mcp.json`:

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

For a repository-specific Cursor setup, use `.cursor/mcp.json` in the project
root with the same `mcpServers` shape. Put the
[Agent Policy Prompt](client-workflow.md#agent-policy-prompt) in your Cursor
rules or paste it into the agent prompt so Cursor uses `project_overview` and
`context_pack` before broad file search.

### Generic MCP JSON Clients

Clients that accept the standard `mcpServers` JSON shape can use:

```json
{
  "mcpServers": {
    "codeinsight": {
      "type": "stdio",
      "command": "/absolute/path/to/codeinsight",
      "args": ["serve", "--transport", "stdio"]
    }
  }
}
```

For clients that do not accept `type`, remove it and keep `command` plus
`args`.

Client MCP configuration surfaces can change between releases. Check the
official client docs when wiring a new environment:

- Codex config reference: https://developers.openai.com/codex/config-reference
- Claude Code MCP guide: https://docs.anthropic.com/en/docs/claude-code/mcp
- Cursor MCP guide: https://cursor.com/docs/mcp

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

1. Call `agent_route` with `root`, `task`, and `token_budget`.
2. Read the returned `context_pack.files[]` in `reading_plan[]` order.
3. Treat `reading_plan[].reason` as the current-step instruction and
   `reading_plan[].selection_reason` as display or audit evidence.
4. Use `continuation_summary` only after selected context is consumed.

Call `index_project`, `project_overview`, `context_pack`, and
`impact_analysis` directly when the client needs custom routing, partial
refresh control, or a user already named a specific file or symbol.

See [First-read workflow](first-read-workflow.md) for the full overview and
context-pack response contract.

`project_overview.recommended_next_tools[]` entries include:

- `tool`: MCP tool name.
- `priority`: display priority; lower numbers should be shown first.
- `reason`: short explanation for surfacing in clients.
- `suggested_arguments`: JSON arguments that can be passed to `tools/call`
  after user/task-specific edits.

See [Recommendation contract](recommendation-contract.md) for shared priority
bands and client sorting guidance.

Example `config_status` call arguments:

```json
{
  "root": "/absolute/path/to/repo"
}
```

`config_status` returns whether `.codeinsight/config.toml` exists, whether it
loaded successfully, any `parse_error`, configured JavaScript package
conditions, configured impact-analysis commands, detected fallback test
commands, and `commands_override_builtin` so clients can explain whether
configured commands will take precedence over built-in inference.

`context_pack` returns `files[]` entries with structured `source`, `score`,
`reason`, and `ranges[]` fields. Each range also includes `source`, `score`,
`start_line`, `end_line`, `importance`, `reason`, and `excerpt`, so clients can
sort or filter snippets without parsing explanation text.
It also returns `seed_strategy` and `selected_seeds`; use these fields to show
whether context came from explicit seeds, an overview entrypoint, task-matched
source, or indexed source-file fallback. For task-matched seeds,
`selected_seeds[].matched_keywords` names the task terms that matched file paths
or symbol names.

Example `context_pack` response shape:

```json
{
  "seed_strategy": "auto_task_match",
  "selected_seeds": [
    {
      "kind": "file",
      "value": "src/router.ts",
      "source": "task_match",
      "role": "source",
      "matched_keywords": ["router"]
    }
  ],
  "reading_plan": [
    {
      "order": 1,
      "file": "src/auth.ts",
      "focus": "Follow static call graph evidence around the seed flow.",
      "next_action": "follow_call_graph",
      "question": "Which callers or callees explain how control moves through this flow?",
      "reason": "Read this step to answer: Which callers or callees explain how control moves through this flow? If deeper evidence is needed, call impact_analysis. Selection reason: Selected for medium relevance via call_graph",
      "selection_reason": "Selected for medium relevance via call_graph",
      "suggested_tool": {
        "tool": "impact_analysis",
        "priority": 30,
        "reason": "Expand from this file through references, calls, and dependency signals.",
        "suggested_arguments": {
          "root": "/path/to/repo",
          "files": ["src/auth.ts"],
          "limit": 20,
          "depth": 2,
          "format": "summary",
          "evidence_limit": 5
        }
      },
      "source": "call_graph",
      "score": 83,
      "ranges": [
        {
          "start_line": 12,
          "end_line": 16,
          "source": "call_graph",
          "importance": "medium"
        }
      ]
    }
  ],
  "budget": {
    "requested_token_budget": 6000,
    "applied_token_budget": 6000,
    "estimated_tokens": 4210,
    "candidate_files": 9,
    "selected_files": 3,
    "omitted_files": 6,
    "candidate_ranges": 14,
    "selected_ranges": 5,
    "omitted_ranges": 9,
    "truncated": true,
    "truncation_reason": "token_budget_exhausted"
  },
  "continuation_summary": {
    "status": "omitted_candidates_available",
    "message": "3 selected files fit the context budget; 6 candidate files were omitted. Continue with src/session.ts if more context is needed.",
    "next_action": "run_omitted_candidate_context_pack",
    "omitted_candidate_count": 1,
    "first_omitted_file": "src/session.ts",
    "suggested_tool": {
      "tool": "context_pack",
      "priority": 60,
      "reason": "Rebuild a focused context pack around this omitted candidate.",
      "suggested_arguments": {
        "root": "/repo",
        "task": "understand auth flow",
        "files": ["src/session.ts"],
        "token_budget": 4000
      }
    }
  },
  "omitted_candidates": [
    {
      "file": "src/session.ts",
      "source": "reference",
      "score": 60,
      "reason": "Omitted from selected context due to budget or lower rank; top reason: References symbol near line 42",
      "ranges": [
        {
          "start_line": 40,
          "end_line": 44,
          "source": "reference",
          "importance": "medium"
        }
      ],
      "suggested_tool": {
        "tool": "context_pack",
        "priority": 60,
        "reason": "Rebuild a focused context pack around this omitted candidate.",
        "suggested_arguments": {
          "root": "/repo",
          "task": "understand auth flow",
          "files": ["src/session.ts"],
          "token_budget": 4000
        }
      }
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

`reading_plan` is derived from the final selected `files[]` after token-budget
selection. Use it when a client needs an ordered read path without carrying the
full code excerpts. `next_action` is a stable snake_case hint for client
controls or follow-up tool routing, and `question` is a short prompt that can be
shown directly to an agent or user. `reason` is the executable instruction for
the current step: it combines the question, deeper-evidence tool, and selection
rationale. `selection_reason` is the compact raw ranking reason for UI or audit
display. `suggested_tool` contains an MCP-ready `tool`, `priority`, `reason`,
and `suggested_arguments` object for the next local analysis call after reading
that step.
Dependency follow-ups are scoped with the current file in
`suggested_arguments.files` when the suggested tool is `dependency_graph`.

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

For the complete MCP tool list and topic-specific contract links, see
[MCP tools](mcp-tools.md).

For an end-to-end protocol check before configuring a GUI client, see
[MCP client smoke test](mcp-client-smoke.md).
