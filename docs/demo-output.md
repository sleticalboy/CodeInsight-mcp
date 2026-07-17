# Two-Minute Demo Output Snapshot

This snapshot captures the expected user-facing shape of
`scripts/two-minute-demo.sh` when it is run from the CodeInsight repository.
Use it as a copyable reference for README videos, project introductions, and
release checks.

The numbers below are from the current repository state on 2026-07-17. Counts
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
   symbols: 931
   duration_ms: <duration_ms>
   errors: 0

2. project_overview
   total_lines: 28235
   entrypoints: 7
   first_entrypoint: src/main.rs
   recommended_next_tools: 4

3. context_pack
   selected_files: 2
   selected_ranges: 4
   reading_plan_steps: 2
   execution_plan_steps: 4
   first_execution_action: read_selected_context
   second_execution_action: use_current_reading_step_suggested_tool
   first_execution_suggested_tool: file_outline
   first_next_action: inspect_seed_file
   first_reading_question: What entrypoints, exported symbols, or setup code define the main flow here?
   selected_lines: 123
   line_reduction: 99.6%
   estimated_tokens: 1073
   continuation: complete
   first_context_file: src/tools.rs
   reading_plan_reason: Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs
   selection_reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs
   route_reason: selected 2 files, 4 ranges, and 2 reading-plan steps within the token budget; read src/tools.rs first via inspect_seed_file, use file_outline when deeper evidence is needed, then follow continuation read_selected_context

4. impact_analysis
   seed_file: src/tools.rs
   risk_level: high
   impacted_files: 7
   paths: 28
   suggested_checks: 4
   route_reason: after selected context is read, pre-edit impact check estimated 7 impacted files at high risk, including 5 call-related files, 1 dependency-related files, 27 call paths, and 1 dependency paths

Run against another repository:
  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh

[Evidence summary]
agent_route selected 123/28235 source lines (99.6% reduction) across 2 files.
First reading question: What entrypoints, exported symbols, or setup code define the main flow here?
The first selected file is src/tools.rs; read it before offering file_outline.
Before edits, impact_analysis reports high risk across 7 impacted files.

[Talk track]
1. agent_route ran index_project, project_overview, context_pack, and impact_analysis in one call.
2. project_overview found 7 entrypoints and 4 recommended next tools.
3. context_pack selected 2 files and 4 ranges, then produced 2 reading-plan steps.
4. execution_plan starts with read_selected_context, then use_current_reading_step_suggested_tool; this keeps suggested tools behind selected-context reading.
5. The first execution-plan suggested tool is file_outline; offer it only after the selected file has been read.
6. The first reading-plan question is: What entrypoints, exported symbols, or setup code define the main flow here?
7. The first reading-plan action is inspect_seed_file; Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs
8. The selected context reduced source reading by 99.6%; selected 2 files, 4 ranges, and 2 reading-plan steps within the token budget; read src/tools.rs first via inspect_seed_file, use file_outline when deeper evidence is needed, then follow continuation read_selected_context
9. Selection evidence: Selected for high relevance via seed_file: Seed file header and imports for task: src/tools.rs
10. Continuation status is complete, so the agent knows whether to ask for a focused follow-up.
11. impact_analysis reports high risk across 7 impacted files with 4 suggested checks; after selected context is read, pre-edit impact check estimated 7 impacted files at high risk, including 5 call-related files, 1 dependency-related files, 27 call paths, and 1 dependency paths

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
  first execution suggested tool, first reading question, executable
  reading-plan reason, raw selection reason, token estimate, line reduction,
  route reason, and continuation status.
- `impact_analysis` includes its route reason so the demo frames it as the
  pre-edit impact check after selected context is read.
- The evidence summary gives a compact copyable result for README videos or
  project introductions.
- The talk track explains the same path a user should show in a recording.
- The agent policy matches the MCP client workflow.

Refresh this snapshot after material changes to indexing, project overview,
context packing, impact analysis, or demo wording.
