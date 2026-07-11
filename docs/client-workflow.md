# Client Workflow

This document describes how an MCP client or agent should consume CodeInsight
for a multi-step code-reading task.

## Standard Flow

1. Call `index_project` for the repository.
2. Call `project_overview`.
3. Execute the first suitable `project_overview.recommended_next_tools[]`
   entry, usually `context_pack`.
4. Read `context_pack.files[]` by following `reading_plan[]` order.
5. Execute `reading_plan[].suggested_tool` when a selected file needs deeper
   local navigation.
6. Use `continuation_summary` and `omitted_candidates[]` when more context is
   needed after the first selected pack.
7. Use `impact_analysis` before edits or refactors to estimate local change
   radius and suggested checks.

## Project Overview

`project_overview` is the repository briefing. Clients should render:

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

1. Run `index_project`.
2. Run `project_overview`.
3. Run the top `context_pack` recommendation.
4. Present selected `files[]` in `reading_plan[]` order.
5. Execute the current step's `suggested_tool` when the user asks for detail.
6. If the selected context is insufficient, execute
   `continuation_summary.suggested_tool` when present.
7. Run `impact_analysis` before edits.

For field-level contracts, see [First-read workflow](first-read-workflow.md)
and [Recommendation contract](recommendation-contract.md).
