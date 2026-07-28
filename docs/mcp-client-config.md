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

## First Agent Route Call

After the MCP server is configured, the first broad repository task should call
`agent_first_read` through MCP `tools/call`:

```json
{
  "name": "agent_first_read",
  "arguments": {
    "root": "/absolute/path/to/repo",
    "task": "understand the main application entrypoint",
    "token_budget": 6000
  }
}
```

`agent_first_read` always returns compact structured content, defers impact
analysis, and applies an 8000-token structured response budget by default. Use
advanced `agent_route` when another local graph backend has already produced
advisory evidence, or when the client explicitly needs the full overview and a
synchronous impact preview.

To let `agent_first_read` invoke an installed `codebase-memory-mcp` binary
directly, add the optional backend configuration:

```json
{
  "name": "agent_first_read",
  "arguments": {
    "root": "/absolute/path/to/repo",
    "task": "understand the main application entrypoint",
    "token_budget": 6000,
    "backend": {
      "provider": "codebase-memory-mcp",
      "on_failure": "fallback_local"
    }
  }
}
```

The existing backend graph is reused. A missing project is indexed in fast
mode automatically. `on_failure` defaults to `fallback_local`, so missing
binaries, command failures, timeouts, and invalid backend responses do not
block the standalone local route. Set it to `error` for strict behavior.
Invalid backend configuration always returns an MCP error. The structured
`backend_status.status` is `used`, `no_candidates`, `fallback_local`, or
`skipped_explicit_seed`; the latter status avoids a redundant backend process
when `files` or `symbols` already provide an exact seed, while
`fallback_local` includes a bounded `reason` for diagnostics.

The response is the default first-read bundle. A minimal client should:

1. If `context_pack.continuation_summary.status` is `blocked_no_seed`, ask the
   user for a seed file or symbol and retry `agent_first_read`; do not fall back to
   broad repository reads.
2. Read `context_pack.files[]` in `context_pack.reading_plan[]` order.
3. Treat `context_pack.reading_plan[].focus` as the compact scan label and
   `context_pack.reading_plan[].question` as the local checklist for the
   selected file.
4. Follow `agent_first_read.execution_plan[]` as the UI or agent checklist.
5. Offer `execution_plan[].suggested_tool` only after the selected file has
   been read.
6. Run the deferred `impact_analysis` suggested by the final execution step
   before edits.

UI gating rules:

- Mark `read_selected_context` as the first active step.
- Keep `execution_plan[].suggested_tool` disabled or visually secondary until
  the matching selected context file has been read.
- Keep `continuation_summary.suggested_tool` hidden, disabled, or visually
  secondary until all selected context needed for the task has been consumed.
- Surface `impact_analysis` as the pre-edit review step, not as proof that an
  edit is safe.

Expected first-call signals:

| Field | Expected Signal | Client Action |
| --- | --- | --- |
| `response_mode` | Is `compact`. | Consume `structuredContent` instead of parsing the concise text summary. |
| `response_budget` | Reports the requested and estimated structured-response tokens. | Treat `omitted_excerpts` as a signal to use the returned range coordinates or focused follow-up tools. |
| `backend_status` | Reports `used`, `no_candidates`, `fallback_local`, or `skipped_explicit_seed` when automatic backend invocation was requested. | Continue with returned local context on fallback or explicit-seed skip, and surface `reason` only as a diagnostic. |
| `context_pack.files[]` | Contains the bounded files or excerpts to read first. | Read these files before broad `rg` or full-file scans. |
| `agent_first_read.current_reading_step` | Mirrors `context_pack.reading_plan[0]` when a reading plan exists. | Use it as the first checklist row without rebuilding it from nested fields. |
| `context_pack.reading_plan[].focus` | Gives the compact scan label for the selected file. | Show it beside the file path or current step. |
| `context_pack.reading_plan[].question` | States the concrete question the selected file should answer. | Show it as the local reading checklist. |
| `context_pack.reading_plan[].reason` | Explains what the agent should learn from the selected file. | Show it as the current reading instruction. |
| `context_pack.reading_plan[].selection_rank` | Preserves the file's rank from the candidate list that produced the selected pack. | Show it in logs or UI when explaining why this file came first. |
| `context_pack.reading_plan[].selection_reason` | Explains why this file was selected under the token budget. | Use it as compact evidence in logs or UI. |
| `context_pack.continuation_summary.status` | Can be `blocked_no_seed` when no source seed can be inferred. | Ask for a seed file or symbol and retry instead of broad-reading the repository. |
| `context_pack.continuation_summary.next_action` | Gives the next post-read action after selected context is consumed. | Use it only after the selected context is read. |
| `execution_plan[]` | Starts with `read_selected_context`, then gates deeper tools and continuation. | Render it as the ordered checklist for the agent. |
| `execution_plan[].suggested_tool` | Contains a ready MCP tool call such as `file_outline` when deeper local structure is useful. | Run it only after the related selected context has been read. |
| `impact_status` | Is `deferred_by_request` when context was selected. | Run the suggested `impact_analysis` before editing. |

Use `scripts/mcp-stdio-smoke.sh` to verify this path end to end. The smoke
checks the bounded `agent_first_read` response and also verifies that the
advanced route's `execution_plan[].suggested_tool` executes through MCP
`tools/call` with first-reading rank and continuation evidence.

For a shorter copyable check, run `scripts/mcp-first-call-smoke.sh`. It prints
a JSON summary with `route_tools`, `selected_files`, `execution_plan_actions`,
the first context file, first reading selection rank, current-reading-step
mirror check, `context_pack_read_less`, `reading_plan[]`, continuation summary
fields, suggested-tool handoff checks, current-step instruction action/question checks,
`suggested_tool_executed`, and
`impact_status`.

Expected summary shape:

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
  "selected_files": ["src/auth.ts", "src/audit.ts"],
  "seed_strategy": "auto_task_path",
  "selected_seeds": [
    {
      "kind": "file",
      "role": "source",
      "source": "task_path",
      "value": "src/auth.ts"
    }
  ],
  "first_seed_source": "task_path",
  "first_seed_value": "src/auth.ts",
  "first_context_file": "src/auth.ts",
  "first_reading_file": "src/auth.ts",
  "first_reading_selection_rank": 1,
  "current_reading_step_matches_reading_plan": true,
  "context_pack_read_less": {
    "baseline_source_lines": 18,
    "selected_source_lines": 10,
    "source_lines_avoided": 8,
    "line_reduction": "44.4%",
    "read_less_ratio": "1.8x"
  },
  "baseline_source_lines": 18,
  "selected_source_lines": 10,
  "source_lines_avoided": 8,
  "line_reduction": "44.4%",
  "read_less_ratio": "1.8x",
  "reading_plan": [
    {
      "file": "src/auth.ts",
      "selection_rank": 1,
      "next_action": "inspect_seed_file",
      "focus": "Start with seed file authentication and session boundaries.",
      "question": "Where are authentication decisions, credentials, or session boundaries handled here?",
      "reason": "Read this step to answer: Where are authentication decisions, credentials, or session boundaries handled here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file",
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
      "path": "/absolute/path/to/repo/src/auth.ts"
    }
  },
  "suggested_tool_executed": true,
  "impact_status": "complete"
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

1. Call `agent_first_read` with `root`, `task`, and `token_budget`.
2. If `context_pack.continuation_summary.status` is `blocked_no_seed`, ask for
   a seed file or symbol and retry `agent_first_read`; do not broad-read the
   repository.
3. Follow `agent_first_read.execution_plan[]`: read selected context first, use the
   current reading step's `suggested_tool` only when needed, inspect
   `continuation_summary` after selected context, then review impact before
   edits.
4. Read the returned `context_pack.files[]` in `reading_plan[]` order.
5. Use `agent_first_read.current_reading_step` for the first checklist row. Treat
   `reading_plan[].focus` as the compact scan label,
   `reading_plan[].question` as the local checklist,
   `reading_plan[].reason` as the current-step instruction, and
   `reading_plan[].selection_rank` plus `reading_plan[].selection_reason` as
   display or audit evidence.
6. Use `continuation_summary` only after selected context is consumed.

Call `index_project`, `project_overview`, `context_pack`, and
`impact_analysis` directly when the client needs custom routing, partial
refresh control, or a user already named a specific file or symbol.

See [First-read workflow](first-read-workflow.md) for the full overview and
context-pack response contract. See
[Client integration examples](client-integration-examples.md) for copyable
Codex, Claude Code, Cursor, and generic MCP consumption policies.

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
conditions, configured index include/exclude globs, configured impact-analysis
commands, detected fallback test commands, and `commands_override_builtin` so
clients can explain whether configured commands will take precedence over
built-in inference.

`context_pack` returns `files[]` entries with structured `source`, `score`,
`selection_rank`, `reason`, and `ranges[]` fields. Each range also includes
`source`, `score`, `start_line`, `end_line`, `importance`, `reason`, and
`excerpt`, so clients can sort or filter snippets without parsing explanation
text.
It also returns `seed_strategy` and `selected_seeds`; use these fields to show
whether context came from explicit seeds, an overview entrypoint, task-matched
source, or indexed source-file fallback. For task-matched seeds,
`selected_seeds[].matched_keywords` names the task terms that matched file paths
or symbol names, and `selected_seeds[].matched_symbols` names the strongest
matched symbols from that seed file. `agent_route` also forwards those symbols
into `impact_seed_symbols` when the caller did not provide explicit symbols, so
the pre-edit `impact_analysis` preview starts from the matched implementation
instead of only the file path. The first seed remains the task match; a later
seed can be an `overview_entrypoint` companion that preserves the application
startup path.

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
      "matched_keywords": ["router"],
      "matched_symbols": ["registerRoutes", "routeRequest"]
    },
    {
      "kind": "file",
      "value": "src/main.ts",
      "source": "overview_entrypoint",
      "role": "source",
      "matched_keywords": [],
      "matched_symbols": ["main"]
    }
  ],
  "reading_plan": [
    {
      "order": 1,
      "file": "src/auth.ts",
      "selection_rank": 1,
      "focus": "Follow call graph evidence for authentication and session flow.",
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
      "selection_rank": 4,
      "omission_reason": "token_budget_exhausted",
      "next_action": "run_omitted_candidate_context_pack",
      "reason": "Omitted from selected context because token_budget_exhausted; candidate rank 4 by score; top reason: References symbol near line 42",
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
full code excerpts. `selection_rank` preserves the file's rank from the original
candidate ordering, while `order` is the final reading order. `next_action` is a
stable snake_case hint for client controls or follow-up tool routing, and
`question` is a short prompt that can be shown directly to an agent or user.
`reason` is the executable instruction for the current step: it combines the
question, deeper-evidence tool, and selection rationale. `selection_reason` is the compact raw ranking reason
for UI or audit display. `suggested_tool` contains an MCP-ready `tool`,
`priority`, `reason`, and `suggested_arguments` object for the next local
analysis call after reading that step.
Dependency follow-ups are scoped with the current file in
`suggested_arguments.files` when the suggested tool is `dependency_graph`.

`omitted_candidates[]` uses the same candidate ranking scale. Its
`selection_rank`, `omission_reason`, and `next_action` fields let clients show
why an important-looking file was excluded and offer the focused
`suggested_tool` without parsing the human-readable `reason`.

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
