# Client Workflow

This document describes how an MCP client or agent should consume CodeInsight
for a multi-step code-reading task.

## Standard Flow

1. Call `agent_route` with `root`, `task`, and `token_budget` for the default
   first read.
2. Read `context_pack.files[]` by following `reading_plan[]` order in the
   returned route payload.
3. Execute `reading_plan[].suggested_tool` when a selected file needs deeper
   local navigation.
4. Use `continuation_summary` and `omitted_candidates[]` when more context is
   needed after the first selected pack.
5. Use the included `impact_analysis` preview before edits or refactors, then
   run focused `impact_analysis` calls when the target changes.

When a client needs custom routing or partial refresh control, it can call the
lower-level tools directly: `index_project`, `project_overview`,
`context_pack`, then `impact_analysis`.

## Agent Policy Prompt

Use this policy in MCP client instructions or agent system prompts when
CodeInsight is available. For copy-paste task variants, see
[Agent prompt templates](agent-prompt-template.md).

```text
When working in a repository with CodeInsight MCP available:

1. Before broad code reading, call agent_route with root, task, and
   token_budget for the default first read.
2. Read context_pack.files in reading_plan order. Treat reading_plan questions
   as the local reading checklist.
3. Prefer reading_plan[].suggested_tool for deeper evidence on the current
   file. Prefer continuation_summary.suggested_tool only after the selected
   context has been consumed.
4. If continuation_summary.status is complete, do not fetch more context unless
   the user asks a narrower follow-up or the selected context does not answer
   the task.
5. Before editing, review the included impact_analysis preview. If the edit
   target differs from the first-read seed, call impact_analysis with the
   selected files or symbols and run or report the suggested_checks that apply.
6. Use index_project, project_overview, context_pack, and impact_analysis
   directly only when custom routing or partial refresh control is needed.
7. Treat CodeInsight call graphs and references as best-effort navigation
   evidence, not compiler-grade proof.
```

The intent is to reduce blind `rg` / `cat` exploration. Agents should still use
normal file reads when CodeInsight points to a file or when the user requests a
specific source location.

## Task Routing Matrix

| User intent | First CodeInsight call after indexing | Follow-up rule |
| --- | --- | --- |
| "Understand this repo" | `agent_route` | Read `reading_plan[]`; continue only if `continuation_summary` suggests it. |
| "Where is the entrypoint?" | `project_overview` | Inspect `entrypoints[]`; call `context_pack` for the highest-confidence source entrypoint when needed. |
| "Explain this module/file" | `context_pack` with `files[]` set to the named file | Use `file_outline` from `reading_plan[].suggested_tool` for local structure. |
| "Explain this class/function" | `symbol_search`, then `context_pack` with the symbol | Use `callers` or `callees` only when the task asks about flow or dependencies. |
| "What happens if I change this?" | `impact_analysis` with the file or symbol | Review `risk_level`, `impact_counts`, `impacted_files`, `paths`, and `suggested_checks`. |
| "Find references" | `find_references` | Use `context_pack` with selected files if references need surrounding context. |
| "Trace calls" | `callers` or `callees` | Use `impact_analysis` when the trace should become edit-planning evidence. |
| "Need more context" | `continuation_summary.suggested_tool` when present | Prefer omitted-candidate follow-ups after selected context, not before. |

## Project Overview

`agent_route` returns the default first-read bundle: `index_report`,
`overview`, `context_pack`, route metadata, and an optional `impact_analysis`
preview. Clients should use it when the user asks to understand a repository or
begin a broad task.

`project_overview` is the lower-level repository briefing. Clients should render:

- `summary`
- `main_directories`
- `entrypoints`
- `recommended_next_tools`
- `index_status`

Use `recommended_next_tools[]` for the first actionable calls. Sort by
`priority`, preserving response order for equal priority values. The default
first-read recommendation is a `context_pack` call with a repository `root`,
task text, and token budget.

## First Context Pack

For first reads, call `context_pack` with `root`, `task`, and `token_budget`.
Omit `symbols` and `files` unless the user already named a specific entrypoint,
symbol, or file.

Clients should render these fields:

- `summary`
- `seed_strategy`
- `selected_seeds`
- `semantic_status`
- `budget`
- `continuation_summary`
- `reading_plan`
- `files`

Treat `files[]` as the selected context payload. Treat `reading_plan[]` as the
ordered path for reading that payload. It contains no excerpts, so it is safe to
show as navigation and routing metadata.

## Reading Selected Context

For each `reading_plan[]` step:

1. Show `file`, `focus`, `question`, and `ranges[]`.
2. Read the matching excerpts from `files[]`.
3. Offer `suggested_tool` when the user or agent needs deeper evidence.

Common suggested tools:

- `file_outline` for seed files or symbol definitions.
- `impact_analysis` for references and call graph expansion.
- `dependency_graph` for dependency-driven context.
- File-scoped `context_pack` for semantic or fallback continuation.

Suggested tools are MCP-ready, but clients may still adjust task text, limits,
or file filters when the user asks a narrower question.

## Continuing After Budget Limits

Use `continuation_summary` as the compact UI decision point after the selected
context is read.

Important statuses:

- `complete`: read the selected context first; no continuation is required.
- `omitted_candidates_available`: offer the included `suggested_tool` as a
  "continue" action.
- `token_budget_exhausted`: ask for a larger budget or a narrower task.
- `minimum_budget_applied`: continue with the selected context; the server used
  its minimum budget.
- `lower_ranked_context_omitted`: narrow the task or seed if the omitted lower
  ranked context matters.

`omitted_candidates[]` lists bounded, excerpt-free follow-up candidates. Use it
after `reading_plan[]`, not before the selected context. Each entry includes
range metadata and a focused `context_pack` call.

## Before Editing

Before making code changes, call `impact_analysis` with the selected files or
symbols. Render:

- `risk_level`
- `impact_counts`
- `impacted_files`
- `paths`
- `suggested_checks`

Use `suggested_checks[]` to decide which local commands or review steps to run.
Recommendation priority does not imply safety. Risk comes from
`impact_analysis` evidence, not from `context_pack` ranking.

## Minimal Client Policy

A simple client can implement this policy:

1. Run `agent_route`.
2. Present selected `files[]` in `reading_plan[]` order.
3. Execute the current step's `suggested_tool` when the user asks for detail.
4. If the selected context is insufficient, execute
   `continuation_summary.suggested_tool` when present.
5. Use the included `impact_analysis` preview before edits, and rerun
   `impact_analysis` for changed targets.

For field-level contracts, see [First-read workflow](first-read-workflow.md)
and [Recommendation contract](recommendation-contract.md).
