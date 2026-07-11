# First-Read Workflow

This document describes the recommended CodeInsight flow for helping an agent
understand a repository before deeper navigation or edits.

## MCP Flow

Recommended MCP first-read flow:

1. Call `index_project` for the repository.
2. Call `project_overview` to inspect summary, roles, entrypoint candidates,
   and recommended next tools.
3. Call `context_pack` with `root`, `task`, and `token_budget`. Omit `symbols`
   and `files` to let CodeInsight auto-select the highest-confidence source
   entrypoint.

For client setup snippets, see [MCP client configuration](mcp-client-config.md).
For recommendation fields and priorities, see
[Recommendation contract](recommendation-contract.md).

## Project Overview

`project_overview` / `overview` returns the indexed repository briefing an
agent should fetch before deeper tools. It preserves the basic file, symbol,
language, and top-directory stats, and adds:

- `summary`
- `total_lines`
- `main_directories`
- `symbol_kinds`
- `dependency_summary`
- `call_summary`
- `entrypoints`
- `recommended_next_tools`
- `index_status`

`main_directories` and `entrypoints` include role hints such as `source`,
`test`, `fixture`, `vendor`, `docs`, or `example`. Entrypoints are heuristic
candidates based on conventional file names and entry-like symbols such as
`main`, with a normalized `confidence` score.

`recommended_next_tools` contains MCP-ready `tool`, `priority`, `reason`, and
`suggested_arguments` entries for likely next calls such as `context_pack`,
`dependency_graph`, `impact_analysis`, and `config_status`. `dependency_graph`
recommendations include a source-entrypoint `files` filter when one is
available. Lower `priority` values should be displayed first.

## Context Pack

`context_pack` combines symbol search, file seeds, reference search, static call
graph hints, resolved local dependencies, semantic vector matches when the
configured provider has indexed vectors, and local semantic chunk fallback
matches into a token-budgeted context bundle for agents.

If no `symbols` or `files` are provided, it uses `project_overview` entrypoint
candidates to auto-select the highest-confidence `source` entrypoint. If no
entrypoint exists, it falls back to indexed source files. Test, fixture, vendor,
docs, and example files are not auto-selected unless the task explicitly asks
for those roles.

The response includes `seed_strategy` (`explicit`, `auto_entrypoint`, or
`auto_source_fallback`) and `selected_seeds` so clients can inspect seed
decisions without parsing summary text.

## Context Ranking

`context_pack` ranks candidates before applying the token budget:

- Explicit file seeds.
- Symbol definitions.
- Call graph targets.
- References.
- Semantic matches.
- Resolved local dependencies.

Task keywords provide a lightweight relevance boost. Inferred ranges from test
and fixture files are downranked by default, but promoted when the task asks for
tests, specs, coverage, regression, or when an explicit seed file is test-like.

File seeds include header/import context and primary top-level symbols instead
of blindly copying the first chunk of a file. Oversized seed ranges can be
shortened to fit small budgets.

Returned ranges include `source`, `score`, `reason`, and `excerpt`, are trimmed
to avoid duplicate lines, and are ordered by source line within each file.
File-level `source` and `reason` values identify the dominant selected source,
and file-level `score` is the highest selected range score.

`context_pack` returns a `budget` object alongside the legacy top-level
`estimated_tokens` and `truncated` fields. Use `requested_token_budget`,
`applied_token_budget`, `candidate_files`, `selected_files`, `omitted_files`,
`candidate_ranges`, `selected_ranges`, `omitted_ranges`, and
`truncation_reason` to explain why a large repository context was shortened and
whether a follow-up, narrower `context_pack` call is useful.

When high-ranked files are omitted entirely, `omitted_candidates` returns a
bounded, excerpt-free list of the next files to inspect. Each entry includes
range metadata and a `suggested_tool` with a focused `context_pack` call so MCP
clients can continue without asking the model to invent follow-up arguments.

Known source values include:

- `seed_file`
- `symbol_definition`
- `reference`
- `call_graph`
- `semantic`
- `dependency`

## Reading Plan

`reading_plan` provides an ordered, excerpt-free read path over the selected
files with focus text, machine-readable `next_action`, guiding `question`,
prioritized `suggested_tool`, and structured line ranges.

The plan is derived from the final selected `files[]` after token-budget
selection. It is a client hint, not a separate ranking pass. Suggested tool
calls are heuristic routing hints and should be executed through MCP
`tools/call`.
Dependency follow-up suggestions include a file-scoped `dependency_graph`
argument when a selected context file came from local dependency evidence.

## Semantic Status

The `semantic_status` object reports whether vector or fallback semantic ranges
were available and selected, plus the next suggested action. The hybrid ranking
path remains local-first and falls back cleanly when optional embeddings are not
configured.
