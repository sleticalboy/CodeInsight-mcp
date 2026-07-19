# Two-Minute Demo Output Snapshot

This snapshot captures the expected user-facing shape of
`scripts/two-minute-demo.sh` when it is run from the CodeInsight repository.
Use it as a copyable reference for README videos, project introductions, and
release checks.

The numbers below are from the current repository state on 2026-07-19. Counts
and timing may change as the codebase changes; the stable contract is the
four-step agent path and the presence of the routing metrics.

## Command

```bash
scripts/two-minute-demo.sh
```

## Output

```text
CodeInsight two-minute demo

Problem: AI agents waste the first read by scanning broad files and guessing entrypoints.
Promise: route the agent through agent_route before edits.

[Live run]
building release binary...
    Finished `release` profile [optimized] target(s) in <build_time>
CodeInsight agent_route demo
root: <repo>/CodeInsight-mcp
task: understand agent context routing
token_budget: 6000

1. index_project
   indexed_files: 23
   symbols: 981
   duration_ms: <duration_ms>
   errors: 0

2. project_overview
   total_lines: 29931
   entrypoints: 7
   first_entrypoint: src/main.rs
   recommended_next_tools: 4

3. context_pack
   selected_files: 6
   selected_ranges: 11
   reading_plan_steps: 6
   execution_plan_steps: 4
   first_execution_action: read_selected_context
   second_execution_action: use_current_reading_step_suggested_tool
   first_execution_suggested_tool: file_outline
   first_next_action: inspect_seed_file
   first_reading_focus: Start with seed file context and primary symbols.
   first_reading_question: What entrypoints, exported symbols, or setup code define the main flow here?
   first_selection_rank: 1
   selected_lines: 423
   line_reduction: 98.6%
   estimated_tokens: 3675
   continuation: complete
   continuation_next_action: read_selected_context
   first_omitted_candidate: none
   first_context_file: src/tools.rs
   first_reading_file: src/tools.rs
   reading_order_contract: true
   suggested_tool_handoff_contract: true
   continuation_timing_contract: true
   reading_plan_reason: Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs; matched task keywords: agent, context, route, router, routes
   selection_reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs; matched task keywords: agent, context, route, router, routes
   route_reason: selected 6 files, 11 ranges, and 6 reading-plan steps within the token budget; read src/tools.rs first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; no omitted candidate follow-up is needed before the selected context is read; continuation read_selected_context

4. impact_analysis
   seed_file: src/tools.rs
   risk_level: high
   impacted_files: 7
   paths: 39
   suggested_checks: 4
   route_reason: after selected context is read, pre-edit impact check estimated 7 impacted files at high risk, including 5 call-related files, 1 dependency-related files, 38 call paths, and 1 dependency paths

Run against another repository:
  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
Save the raw agent_route JSON:
  CODEINSIGHT_DEMO_SAVE_JSON=/tmp/codeinsight-agent-route.json scripts/two-minute-demo.sh

[Evidence summary]
agent_route selected 423/29931 source lines (98.6% reduction) across 6 files.
First reading focus: Start with seed file context and primary symbols.
First reading question: What entrypoints, exported symbols, or setup code define the main flow here?
The first selected file is src/tools.rs; reading_plan starts at src/tools.rs as candidate rank 1.
Execution contract: reading_order=true, suggested_tool_handoff=true, continuation_after_selected_context=true.
Selection evidence: Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs; matched task keywords: agent, context, route, router, routes
Continuation: status=complete, next_action=read_selected_context.
Next follow-up candidate: none before selected context is read.
Read src/tools.rs before offering file_outline.
Before edits, impact_analysis reports high risk across 7 impacted files.

[Talk track]
1. agent_route ran index_project, project_overview, context_pack, and impact_analysis in one call.
2. project_overview found 7 entrypoints and 4 recommended next tools.
3. context_pack selected 6 files and 11 ranges, then produced 6 reading-plan steps.
4. execution_plan starts with read_selected_context, then use_current_reading_step_suggested_tool; this keeps suggested tools behind selected-context reading.
5. The first execution-plan suggested tool is file_outline; offer it only after the selected file has been read.
6. The first reading-plan focus is: Start with seed file context and primary symbols.
7. The first reading-plan question is: What entrypoints, exported symbols, or setup code define the main flow here?
8. The first reading-plan action is inspect_seed_file; Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs; matched task keywords: agent, context, route, router, routes
9. Reading order contract is true; execution_plan[0].files follows reading_plan[] order.
10. Suggested-tool handoff contract is true; execution_plan[1] points to the current reading step.
11. Continuation timing contract is true; continuation is only considered after selected context is read.
12. The selected context reduced source reading by 98.6%; selected 6 files, 11 ranges, and 6 reading-plan steps within the token budget; read src/tools.rs first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; no omitted candidate follow-up is needed before the selected context is read; continuation read_selected_context
13. Selection evidence: candidate rank 1; Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs; matched task keywords: agent, context, route, router, routes
14. Continuation status is complete; next_action=read_selected_context, so no omitted candidate follow-up is needed before selected context is read.
15. impact_analysis reports high risk across 7 impacted files with 4 suggested checks; after selected context is read, pre-edit impact check estimated 7 impacted files at high risk, including 5 call-related files, 1 dependency-related files, 38 call paths, and 1 dependency paths

[Agent policy]
Call agent_route with root, task, and token_budget for the default first read.
Read context_pack.files in reading_plan order, use continuation_summary only after that, and run focused follow-up tools only when needed.

Run this walkthrough against another repository:
  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

## What To Check

- The path starts with `agent_route`, whose `route[]` includes
  `index_project`, `project_overview`, `context_pack`, and `impact_analysis`.
- `context_pack` includes selected files, selected ranges, reading-plan steps,
  execution-plan steps, first execution action, first next action,
  first execution suggested tool, first reading focus, first reading question,
  first selection rank, executable reading-plan reason, raw selection reason,
  token estimate, line reduction, route reason, continuation status,
  continuation next action, and omitted-candidate status.
- `impact_analysis` includes its route reason so the demo frames it as the
  pre-edit impact check after selected context is read.
- The evidence summary gives a compact copyable result for README videos or
  project introductions.
- The talk track explains the same path a user should show in a recording.
- The agent policy matches the MCP client workflow.
- `CODEINSIGHT_DEMO_SAVE_JSON` can persist the raw `agent_route` payload for
  issue reports, README recordings, benchmark evidence, or client integration
  debugging.

Refresh this snapshot after material changes to indexing, project overview,
context packing, impact analysis, or demo wording.
