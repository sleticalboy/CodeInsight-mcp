# Client Integration Examples

Use these examples when wiring CodeInsight into Codex, Claude Code, Cursor, or
another MCP-capable coding agent. The configuration starts the server; the
integration policy tells the agent how to consume `agent_route.execution_plan[]`
without falling back to broad file scans.

For server setup snippets, see [MCP client configuration](mcp-client-config.md).
For the complete field contract, see [Client workflow](client-workflow.md).

## Core Consumption Loop

Every client should treat `agent_route.execution_plan[]` as the ordered action
plan after the server has completed the first-read route.

```text
1. Call agent_route with root, task, and token_budget.
2. Read context_pack.files[] in reading_plan[] order.
3. Use reading_plan[].question as the local checklist for the selected file.
4. Use reading_plan[].reason as the current-step instruction.
5. Use reading_plan[].selection_reason only as selection evidence.
6. Call execution_plan[].suggested_tool only when the current step needs deeper
   evidence.
7. Use continuation_summary only after selected context has been read.
8. Review impact_analysis before edits.
```

Do not treat `route[]` and `execution_plan[]` as the same thing:

- `route[]` explains which tools CodeInsight already ran.
- `execution_plan[]` tells the client or agent what to do next.

## Generic MCP Agent

Use this prompt when a client accepts plain instructions but not repo-specific
instruction files:

```text
Use CodeInsight for repository first reads.

When the task is broad, call agent_route with the repository root, the user's
task, and token_budget 6000. Follow agent_route.execution_plan[] in order:
read_selected_context first, use_current_reading_step_suggested_tool only when
the current file needs deeper evidence, use_continuation_if_needed only after
selected context is consumed, and review_impact_before_edits before changing
code.

Read context_pack.files[] in reading_plan[] order. Treat
reading_plan[].question as the local checklist for the selected file,
reading_plan[].reason as the instruction for the current file, and
reading_plan[].selection_reason as evidence for why the file was selected, not
as a replacement for question or reason.
```

## Codex

Add the MCP server in `~/.codex/config.toml` as shown in
[MCP client configuration](mcp-client-config.md#codex). Then put this policy in
the repository `AGENTS.md`:

```text
When CodeInsight MCP is available, call agent_route before broad repository
reading. Follow agent_route.execution_plan[] exactly:

1. read_selected_context: read context_pack.files[] in reading_plan[] order.
2. use_current_reading_step_suggested_tool: use the current step's
   suggested_tool only if the selected file needs deeper evidence.
3. use_continuation_if_needed: inspect continuation_summary only after selected
   context has been read.
4. review_impact_before_edits: review impact_analysis before editing.

Use reading_plan[].question as the local checklist,
reading_plan[].reason as the current-step instruction, and
reading_plan[].selection_reason as selection evidence.
```

## Claude Code

After adding the stdio MCP server, place the same policy in project
instructions or paste it at the start of the session:

```text
Use CodeInsight as the first-read router for this repository. Start broad
questions with agent_route. Follow execution_plan[] before raw repository
search: read selected context first, use the current reading step's
suggested_tool only when needed, use continuation only after selected context,
and review impact before edits.

Summaries should name the files read from context_pack.files[] and mention any
continuation or impact-analysis evidence used.
```

## Cursor

After adding `codeinsight` to Cursor MCP configuration, add this to Cursor rules
or paste it into the agent prompt:

```text
For repository-understanding tasks, prefer CodeInsight agent_route before broad
file search. Use agent_route.execution_plan[] as the UI/agent checklist:
read_selected_context -> use_current_reading_step_suggested_tool ->
use_continuation_if_needed -> review_impact_before_edits.

Only offer continuation_summary.suggested_tool after selected context has been
read. Use question for the local checklist, reason for the agent's reading
instruction, and selection_reason for display or audit labels.
```

## UI Checklist

Clients with a visible tool panel should render:

- `execution_plan[].action` as the next-action checklist.
- `execution_plan[].status` as the availability state.
- `execution_plan[].instruction` as the agent-facing instruction.
- `execution_plan[].suggested_tool` as an optional action button that becomes
  active only after the matching selected context file is read.
- `reading_plan[].question` beside each selected file as the local checklist.
- `reading_plan[].reason` beside each selected file.
- `reading_plan[].selection_reason` as compact evidence text.
- `continuation_summary.suggested_tool` as a continue action only after the
  selected context is insufficient for the current task.
- `impact_analysis` as a pre-edit review step before any edit controls are
  treated as ready.

Suggested-tool buttons should be disabled or visually secondary until the
selected file for the current reading step has been consumed.
Continuation buttons should stay disabled or hidden until selected context has
been consumed and the task still needs more evidence.
Impact-review controls should be shown after the first read and before edits;
they should not be labeled as a safety guarantee.

## Acceptance Checks

A working integration should pass these checks:

- The first broad task calls `agent_route`.
- The agent reads selected files in `reading_plan[]` order.
- The agent can answer `reading_plan[].question` for each selected file.
- `read_selected_context` happens before `use_current_reading_step_suggested_tool`.
- `continuation_summary.suggested_tool` is not used before selected context.
- `impact_analysis` is reviewed before edits.
- The final response can explain which selected files were read and why they
  were selected.
