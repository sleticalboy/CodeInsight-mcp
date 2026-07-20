# First-Read Workflow

This document describes the recommended CodeInsight flow for helping an agent
understand a repository before deeper navigation or edits.

## CLI Demo

Run the local demo from the repository root:

```bash
scripts/two-minute-demo.sh
```

Run it against another repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

The script executes the product path that an MCP client should follow:
`agent_route`, which internally runs index, overview, context-pack, and
impact-analysis.

It reports:

- index timing, indexed files, symbols, and errors
- overview entrypoint and recommendation counts
- context-pack selected files, selected ranges, estimated tokens, and
  `read_less` source-line reduction metrics
- first reading-plan question, candidate selection rank, executable reason, and
  raw selection reason for the first selected context file
- continuation status and next action for follow-up context calls
- impact-analysis risk, impacted file count, path count, and suggested checks

## MCP Flow

Recommended MCP first-read flow:

1. Call `agent_route` with `root`, `task`, and `token_budget`.
2. Read `context_pack.files[]` in `reading_plan[]` order from the returned
   route payload. Use `agent_route.current_reading_step` as the first checklist
   row. Treat `reading_plan[].focus` as the compact scan label,
   `reading_plan[].question` as the local checklist for the selected file,
   `reading_plan[].reason` as the executable instruction for the current step,
   `reading_plan[].selection_rank` as the candidate-rank audit trail, and
   `reading_plan[].selection_reason` as compact evidence for why the file was
   selected.
3. Use `continuation_summary.next_action` only after selected context is
   consumed.

The first-read ordering contract is strict:

```text
agent_route -> read selected context -> current-step suggested_tool if needed -> continuation if needed -> impact check before edits
```

`suggested_tool` and continuation are follow-up actions, not replacements for
reading the selected context. `impact_analysis` is the pre-edit planning gate;
it does not prove the change is safe and does not replace tests or compiler
checks.

When the client needs custom routing or partial refresh control, call the
lower-level tools directly: `index_project`, `project_overview`,
`context_pack`, then `impact_analysis`.

## Agent Route

`agent_route` is the default first-read contract. It returns:

- `index_report`
- `overview`
- `context_pack`
- `current_reading_step` mirroring `context_pack.reading_plan[0]` when present
- `impact_analysis` when a file or symbol seed is available
- `route[]` metadata describing the executed tool path and why each stage
  matters for the first read
- `execution_plan[]` with the next client actions: read selected context, use
  current-step `suggested_tool` only when needed, inspect continuation after
  selected context, then review impact before edits
- `impact_seed_files` and `impact_seed_symbols`

Use `agent_route` for broad repository understanding and first-pass planning.
Use the lower-level tools when the user named a specific file, symbol, module,
or when the client needs to refresh only part of the route.

For client setup snippets, see [MCP client configuration](mcp-client-config.md).
For a full client-side read, continue, and edit-preflight sequence, see
[Client workflow](client-workflow.md).
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
Framework-oriented file signals are also included for common first-read
surfaces such as Next.js app router files (`app/page.tsx`,
`app/layout.tsx`, `app/**/route.ts`), Next.js pages bootstrap files
(`pages/_app.tsx`, `pages/_document.tsx`), Rails `config/routes.rb`, and Java
`*Application.java` files. Python web framework signals include `manage.py`,
`asgi.py`, `wsgi.py`, and `urls.py`; C# web application signals include
`Program.cs` and `Startup.cs`.

`recommended_next_tools` contains MCP-ready `tool`, `priority`, `reason`, and
`suggested_arguments` entries for likely next calls such as `context_pack`,
`dependency_graph`, `impact_analysis`, and `config_status`. `dependency_graph`
recommendations include a source-entrypoint `files` filter when one is
available, and type-relation recommendations include `kinds: ["base_type"]`.
Lower `priority` values should be displayed first.

`dependency_summary` also reports type-relation evidence extracted from
declaration-level relationships. `type_relation_edges` counts detected
`extends`, `implements`, base-class, and Rust trait implementation edges.
`top_type_relation_targets[]` lists the most referenced relation targets.
Clients should use these fields as routing hints for `dependency_graph` and
context selection, not as a complete type hierarchy.

## Context Pack

`context_pack` combines symbol search, file seeds, reference search, static call
graph hints, resolved local dependencies, semantic vector matches when the
configured provider has indexed vectors, and local semantic chunk fallback
matches into a token-budgeted context bundle for agents.

If no `symbols` or `files` are provided, it combines `project_overview`
entrypoint candidates with task-matching indexed source files, then auto-selects
the strongest seed. If no entrypoint or task match exists, it falls back to
indexed source files. Test, fixture, vendor, docs, and example files are not
auto-selected unless the task explicitly asks for those roles.

Common agent task aliases are expanded before auto-selection. For example,
`routing` can match `route`, `routes`, or `router`; `authentication` can match
`auth`; `settings` can match `config`; and `startup` can match `Program.cs` or
related startup files. Auto-selected seed files keep their seed order before
dependency, call-graph, reference, or semantic expansion context.

The response includes `seed_strategy` (`explicit`, `auto_entrypoint`,
`auto_task_match`, or `auto_source_fallback`) and `selected_seeds` so clients
can inspect seed decisions without parsing summary text. When `seed_strategy`
is `auto_task_match`, `selected_seeds[].matched_keywords` explains which task
terms matched the selected file path or symbol names. Task-matched first reads
keep the task-matched seed first and may include a source entrypoint companion
seed afterward, so agents can stay focused without losing the application
startup path.

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

`context_pack` returns a `read_less` object so clients can display first-read
compression without recomputing it from ranges:

- `baseline_source_lines`: the indexed repository source-line total.
- `selected_source_lines`: the source lines included in selected
  `context_pack.files[].ranges[]`.
- `source_lines_avoided`: non-negative baseline minus selected lines.
- `line_reduction`: one-decimal percentage, or `n/a` when no baseline exists.
- `read_less_ratio`: one-decimal baseline/selected ratio, or `n/a` when the
  ratio is not meaningful.

`context_pack` also returns a `budget` object alongside the legacy top-level
`estimated_tokens` and `truncated` fields. Use `requested_token_budget`,
`applied_token_budget`, `candidate_files`, `selected_files`, `omitted_files`,
`candidate_ranges`, `selected_ranges`, `omitted_ranges`, and
`truncation_reason` to explain why a large repository context was shortened and
whether a follow-up, narrower `context_pack` call is useful.

`continuation_summary` condenses the budget and omission state into a
client-facing status, message, and `next_action`. When omitted candidates are
available, it repeats the first focused `suggested_tool` so clients can offer a
single "continue" action without interpreting budget counters themselves.

When high-ranked files are omitted entirely, `omitted_candidates` returns a
bounded, excerpt-free list of the next files to inspect. Each entry includes
`selection_rank`, `omission_reason`, `next_action`, range metadata, and a
`suggested_tool` with a focused `context_pack` call so MCP clients can continue
without asking the model to invent follow-up arguments.

Known source values include:

- `seed_file`
- `symbol_definition`
- `reference`
- `call_graph`
- `semantic`
- `dependency`

## Reading Plan

`reading_plan` provides an ordered, excerpt-free read path over the selected
files with candidate `selection_rank`, focus text, machine-readable
`next_action`, guiding `question`, executable `reason`, raw `selection_reason`,
prioritized `suggested_tool`, and structured line ranges.

`reason` is written for the agent loop: it names the question to answer, the
suggested follow-up tool to call when deeper evidence is needed, and the
selection reason. `selection_reason` preserves the raw ranking rationale from
the selected `files[]` entry for clients that want to display or audit only why
the file was chosen.

Client behavior should be strict:

- Read the matching `files[]` excerpts for the current `reading_plan[]` step
  before executing continuation tools.
- Use `question` as the local checklist for what the selected file should
  answer.
- Use `reason` as the agent-facing instruction because it combines the
  question, deeper-evidence tool, and selection rationale.
- Use `selection_reason` for compact UI labels, benchmark tables, or audit
  displays where the action guidance would be too verbose.
- Execute `suggested_tool` only when the current step needs deeper local
  evidence; do not run omitted-candidate continuation before reading the
  selected context.
- Use `omitted_candidates[].selection_rank`, `omission_reason`, and
  `next_action` to explain which ranked files were excluded by the token budget
  or lower-priority selection and how to continue with a focused follow-up.

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
