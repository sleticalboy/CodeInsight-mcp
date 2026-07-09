# Recommendation Contract

CodeInsight exposes local next-step suggestions in two places:

- `project_overview.recommended_next_tools[]`
- `context_pack.reading_plan[].suggested_tool`

Both shapes are designed for MCP clients that want to show or execute likely
next calls without parsing explanation text.

## Shared Tool Shape

Recommendation objects include:

- `tool`: MCP tool name to call.
- `priority`: display priority. Lower numbers should be shown first.
- `reason`: short client-facing explanation.
- `suggested_arguments`: JSON arguments that can be passed to `tools/call`
  after user or task-specific edits.

Clients should sort by `priority`, then preserve response order as a stable
tie-breaker.

## Priority Bands

Current priority bands:

- `10`: first-read or direct inspection, such as `context_pack` or
  `file_outline`.
- `30`: structural expansion from selected context, such as
  `dependency_graph` or `impact_analysis`.
- `40`: change-radius or fallback graph inspection, such as
  `impact_analysis` or `callers`.
- `50`: focused follow-up context rebuilds, such as file-scoped
  `context_pack`.
- `80`: validation and environment checks, such as `config_status`.

The exact values are stable enough for client ordering, but they are not a
semantic score. Treat them as coarse display buckets.

## Overview Recommendations

`project_overview.recommended_next_tools[]` recommends repository-level calls
after indexing. It currently favors:

- `context_pack` first, to build the initial reading context.
- `dependency_graph` when dependency edges exist.
- `impact_analysis` when a source entrypoint is detected.
- `callers` as a fallback when call edges exist but no source entrypoint is
  detected.
- `config_status` to reveal project-specific validation commands.

These recommendations are generated from the current index and can be executed
through MCP `tools/call`. The stdio smoke test executes selected overview
recommendations to keep argument shapes valid.

## Reading Plan Recommendations

`context_pack.reading_plan[].suggested_tool` recommends the next local analysis
call after reading a specific step. It is derived from the final selected
context after token-budget filtering, so it reflects what the agent actually
received.

Current mappings:

- `inspect_seed_file` and `inspect_symbol_definition` -> `file_outline`
- `follow_call_graph` and `inspect_references` -> `impact_analysis`
- `inspect_dependency` -> `dependency_graph`
- `review_semantic_matches` -> file-scoped `context_pack`
- fallback -> file-scoped `context_pack`

`next_action`, `question`, and `suggested_tool` are heuristic routing hints.
They do not prove that the related graph, dependency, or semantic view is
complete.

## Client Guidance

Recommended client behavior:

- Render `reason` for users.
- Sort recommendations by `priority`.
- Validate or adjust `suggested_arguments` when the user asks a narrower task.
- Execute suggested calls through MCP `tools/call`.
- Keep original response order for equal priority values.

Do not infer repository safety or change risk from recommendation priority.
Use `impact_analysis.risk_level`, `impact_counts`, evidence, and
`suggested_checks` for change-risk workflows.
