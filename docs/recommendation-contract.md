# Recommendation Contract

CodeInsight exposes local next-step suggestions in two places:

- `agent_route.route[]`
- `agent_route.execution_plan[]`
- `project_overview.recommended_next_tools[]`
- `context_pack.reading_plan[].suggested_tool`
- `context_pack.omitted_candidates[].suggested_tool`

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
- `60`: omitted-context follow-ups, such as rebuilding context around a
  high-ranked file that did not fit the original token budget.
- `80`: validation and environment checks, such as `config_status`.

The exact values are stable enough for client ordering, but they are not a
semantic score. Treat them as coarse display buckets.

## Overview Recommendations

`agent_route.route[]` reports the default first-read path that was executed in
one call. It is route metadata rather than a recommendation list; clients should
render it as provenance for `index_report`, `overview`, `context_pack`, and
`impact_analysis`. The `reason` field also summarizes why the next stage matters:
the `context_pack` step points to the first `reading_plan` file/action,
candidate `selection_rank`, and omitted-candidate continuation when available.
The `impact_analysis` step frames the included preview as the pre-edit impact
check after selected context is read.

## Agent Route Execution Plan

`agent_route.execution_plan[]` is the machine-readable action sequence for the
client or agent after `agent_route` returns. It is different from `route[]`:

- `route[]` explains what CodeInsight already ran.
- `execution_plan[]` explains what the client or agent should do next.

Each execution step includes:

- `order`: stable one-based order.
- `action`: stable snake_case action name.
- `status`: readiness state for the action.
- `instruction`: client-facing instruction text.
- `files[]`: optional selected files related to the action.
- `suggested_tool`: optional MCP-ready tool call for the action.

`agent_route.current_reading_step` mirrors `context_pack.reading_plan[0]` so a
client can render the first file, focus, question, reason, rank, selection
evidence, and suggested tool without digging through the nested context pack.
When no reading plan is available, the field is omitted.
If `context_pack` cannot infer a source seed from an empty or unsupported
repository, `agent_route` still returns a structured report: the context step
uses `blocked_no_seed`, `context_pack.seed_strategy` is `auto_no_seed`, and
`execution_plan[]` keeps the normal action order with blocked/manual statuses
that tell the client to provide a seed file or symbol.

The default action order is:

1. `read_selected_context`
2. `use_current_reading_step_suggested_tool`
3. `use_continuation_if_needed`
4. `review_impact_before_edits`

Clients should read selected `context_pack.files[]` before enabling or
executing `use_current_reading_step_suggested_tool`, and should not use
`continuation_summary.suggested_tool` until selected context has been consumed.
The first execution step names the first reading file with its
`selection_rank`, first reading focus/question, and `context_pack.read_less`
source-line reduction evidence; the continuation step names the first omitted
candidate, `omission_reason`, and suggested continuation tool when one exists.
The second execution step mirrors the first `reading_plan[]` step's
`suggested_tool` and names that step's `next_action`, focus, and question, so
clients can render the follow-up without reassembling context from separate
fields.
Use `review_impact_before_edits` as a pre-edit checkpoint, not as proof of
compiler-grade safety.

`project_overview.recommended_next_tools[]` recommends repository-level calls
after indexing when a client chooses the lower-level path. It currently favors:

- `context_pack` first, to build the initial reading context.
- `dependency_graph` when dependency edges exist, scoped to the detected source
  entrypoint file when available.
- `dependency_graph` when type-relation edges exist, with
  `suggested_arguments.kinds: ["base_type"]`, so clients can inspect
  base-class, interface, and trait implementation context without mixing in
  ordinary import edges.
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
- `inspect_type_relation` -> file-scoped `dependency_graph` with
  `kinds: ["base_type"]`
- `inspect_dependency` -> file-scoped `dependency_graph`
- `review_semantic_matches` -> file-scoped `context_pack`
- fallback -> file-scoped `context_pack`

`next_action`, `question`, and `suggested_tool` are heuristic routing hints.
They do not prove that the related graph, dependency, or semantic view is
complete.

`context_pack.reading_plan[].question` is the local checklist for what the
selected file should answer. Questions are task-aware when the prompt clearly
names impact/call-path, authentication/session, configuration, startup, or
middleware work; otherwise they use the generic entrypoint/setup, definition,
flow, reference, semantic-match, dependency, or selected-range checklist for
the step.
`context_pack.reading_plan[].focus` is the short scan label for the same step
and follows the same task-aware signals, so clients can show a compact row
without losing the task intent carried by `question`.
`context_pack.reading_plan[].reason` is the executable client-facing
explanation for the step: it states the question to answer, when to use the
suggested tool, and why the file was selected. `selection_reason` preserves
only the raw selection/ranking rationale from `files[]` for audit displays.
Focused `context_pack` suggestions from semantic-match and fallback steps keep
the original task in `suggested_arguments.task` while scoping `files[]` to the
selected file.

## Read-Less Metrics

`context_pack.read_less` is display and reporting evidence for the selected
first-read pack. It is not a recommendation and should not change execution
ordering.

The object includes:

- `baseline_source_lines`: repository source-line baseline from
  `project_overview`.
- `selected_source_lines`: selected source lines from `context_pack.files[]`
  ranges.
- `source_lines_avoided`: non-negative baseline minus selected lines.
- `line_reduction`: formatted first-read reduction percentage.
- `read_less_ratio`: formatted baseline/selected ratio.

Clients can show these metrics beside `reading_plan[]` or in run summaries.
They must still read selected context before using `suggested_tool` or
continuation actions.

## Omitted Candidate Recommendations

`context_pack.continuation_summary` is the compact client-facing view of the
same state. It reports a status, message, next action, omitted candidate count,
and, when available, the first omitted-candidate `suggested_tool`.

`context_pack.omitted_candidates[]` reports high-ranked candidate files that
were excluded from the final selected context. It is emitted after token-budget
selection and intentionally omits code excerpts, so clients can show what was
left out without expanding the response back toward the original repository
size.

Each omitted candidate includes:

- `file`, `source`, `score`, and `reason` for display and ranking context.
- `selection_rank`, `omission_reason`, and `next_action` for machine-readable
  continuation UI and agent routing.
- `ranges[]` with line numbers, source, and importance, but no excerpt.
- `suggested_tool` pointing to a file-scoped `context_pack` call.

Clients should treat omitted candidates as continuation options after the user
or agent has consumed the primary `reading_plan`. They are not a replacement
for the selected context and may represent lower-ranked or budget-excluded
signals.

## Client Guidance

Recommended client behavior:

- Render `reason` for users.
- Sort recommendations by `priority`.
- Validate or adjust `suggested_arguments` when the user asks a narrower task.
- Execute suggested calls through MCP `tools/call`.
- Keep original response order for equal priority values.
- Follow `agent_route.execution_plan[]` for one-call first-read clients.
- Display `context_pack.read_less` as source-line reduction evidence when
  users need to understand why the first read is bounded.
- Prefer `reading_plan[].suggested_tool` while reading selected context, then
  use `omitted_candidates[].suggested_tool` when more context is needed.

Do not infer repository safety or change risk from recommendation priority.
Use `impact_analysis.risk_level`, `impact_counts`, evidence, and
`suggested_checks` for change-risk workflows.
