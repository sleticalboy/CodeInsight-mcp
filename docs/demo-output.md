# Two-Minute Demo Output Snapshot

This snapshot captures the expected user-facing shape of
`scripts/two-minute-demo.sh` when it is run from the CodeInsight repository.
Use it as a copyable reference for README videos, project introductions, and
release checks.

The numbers below are from the current repository state on 2026-07-26. Counts
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
   indexed_files: 150
   symbols: 2266
   duration_ms: <duration_ms>
   errors: 0

2. project_overview
   total_lines: 92726
   entrypoints: 12
   first_entrypoint: src/main.rs
   recommended_next_tools: 5

3. context_pack
   selected_files: 1
   selected_ranges: 11
   reading_plan_steps: 1
   execution_plan_steps: 4
   first_execution_action: read_selected_context
   second_execution_action: use_current_reading_step_suggested_tool
   first_execution_suggested_tool: file_outline
   routing_decision_seed_strategy: auto_task_match
   routing_decision_first_seed: task_match:src/tools.rs
   routing_decision_first_file: src/tools.rs
   routing_decision_first_selection_rank: 1
   routing_decision_suggested_tool: file_outline
   routing_decision_read_less: 99.4%, 168.6x
   routing_decision_continuation: omitted_candidates_available
   routing_decision_impact_status: complete
   routing_decision_quality: high (100/100, 24 evidence signals)
   routing_decision_recommended_action: read_selected_context_then_use_continuation_if_needed
   first_next_action: inspect_seed_file
   first_reading_focus: Start with seed file context routing, first-read handoff, and read-less evidence.
   first_reading_question: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here?
   first_selection_rank: 1
   blind_first_read_lines: 92726
   routed_first_read_lines: 550
   selected_lines: 550
   source_lines_avoided: 92176
   line_reduction: 99.4%
   read_less_ratio: 168.6x
   estimated_tokens: 6000
   continuation: omitted_candidates_available
   continuation_next_action: run_omitted_candidate_context_pack
   first_omitted_candidate: src/main.rs (candidate rank 2)
   first_omitted_reason: token_budget_exhausted
   first_omitted_next_action: run_omitted_candidate_context_pack
   first_context_file: src/tools.rs
   first_reading_file: src/tools.rs
   reading_order_contract: true
   read_less_instruction_contract: true
   current_reading_step_contract: true
   suggested_tool_handoff_contract: true
   continuation_timing_contract: true
   reading_plan_reason: Read this step to answer: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file defines symbol agent_route; matched task keywords: agent, route; evidence mix: seed file x10, call graph x1
   selection_reason: Selected for high relevance via seed_file: Seed file defines symbol agent_route; matched task keywords: agent, route; evidence mix: seed file x10, call graph x1
   route_reason: selected 1 files, 11 ranges, and 1 reading-plan steps within the token budget; read src/tools.rs first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; first omitted candidate src/main.rs (candidate rank 2, reason token_budget_exhausted) can be revisited via run_omitted_candidate_context_pack using context_pack after selected context; continuation run_omitted_candidate_context_pack

4. impact_analysis
   seed_file: src/tools.rs
   risk_level: high
   impacted_files: 8
   paths: 50
   suggested_checks: 4
   route_reason: after selected context is read, pre-edit impact check estimated 8 impacted files at high risk, including 7 call-related files, 3 dependency-related files, 50 call paths, and 0 dependency paths

Run against another repository:
  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
Save the raw agent_route JSON:
  CODEINSIGHT_DEMO_SAVE_JSON=/tmp/codeinsight-agent-route.json scripts/two-minute-demo.sh

[Evidence summary]
Blind first-read baseline: 92726 source lines.
Routed first-read: 550 source lines across 1 files.
Read less: avoided 92176 source lines, 168.6x less text before follow-up tools.
Routing decision: seed=task_match:src/tools.rs, first_file=src/tools.rs, rank=1, tool=file_outline, continuation=omitted_candidates_available, impact=complete.
Route quality: high (100/100) from 24 evidence signals; next=read_selected_context_then_use_continuation_if_needed.
agent_route selected 550/92726 source lines (99.4% reduction) across 1 files.
First reading focus: Start with seed file context routing, first-read handoff, and read-less evidence.
First reading question: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here?
The first selected file is src/tools.rs; reading_plan starts at src/tools.rs as candidate rank 1.
Execution contract: reading_order=true, read_less_instruction=true, current_reading_step=true, suggested_tool_handoff=true, continuation_after_selected_context=true.
Selection evidence: Selected for high relevance via seed_file: Seed file defines symbol agent_route; matched task keywords: agent, route; evidence mix: seed file x10, call graph x1
Continuation: status=omitted_candidates_available, next_action=run_omitted_candidate_context_pack.
Next follow-up candidate: src/main.rs at candidate rank 2; token_budget_exhausted; next_action=run_omitted_candidate_context_pack.
Read src/tools.rs before offering file_outline.
Before edits, impact_analysis reports high risk across 8 impacted files.

[Talk track]
1. agent_route ran index_project, project_overview, context_pack, and impact_analysis in one call.
2. project_overview found 12 entrypoints and 5 recommended next tools.
3. context_pack selected 1 files and 11 ranges, then produced 1 reading-plan steps.
4. execution_plan starts with read_selected_context, then use_current_reading_step_suggested_tool; this keeps suggested tools behind selected-context reading.
5. The first execution-plan suggested tool is file_outline; offer it only after the selected file has been read.
6. routing_decision summarizes the same choice: seed=task_match:src/tools.rs, first_file=src/tools.rs, rank=1, read_less=99.4%/168.6x.
7. route_quality is high (100/100) from 24 evidence signals; recommended_action=read_selected_context_then_use_continuation_if_needed.
8. The first reading-plan focus is: Start with seed file context routing, first-read handoff, and read-less evidence.
9. The first reading-plan question is: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here?
10. The first reading-plan action is inspect_seed_file; Read this step to answer: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file defines symbol agent_route; matched task keywords: agent, route; evidence mix: seed file x10, call graph x1
11. Reading order contract is true; execution_plan[0].files follows reading_plan[] order.
12. Read-less instruction contract is true; execution_plan[0].instruction carries selected lines, baseline lines, avoided lines, and read-less ratio.
13. Current reading step contract is true; agent_route.current_reading_step mirrors reading_plan[0].
14. Suggested-tool handoff contract is true; execution_plan[1] points to the current reading step.
15. Continuation timing contract is true; continuation is only considered after selected context is read.
16. The selected context avoided 92176 source lines (99.4%, 168.6x less text); selected 1 files, 11 ranges, and 1 reading-plan steps within the token budget; read src/tools.rs first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; first omitted candidate src/main.rs (candidate rank 2, reason token_budget_exhausted) can be revisited via run_omitted_candidate_context_pack using context_pack after selected context; continuation run_omitted_candidate_context_pack
17. Selection evidence: candidate rank 1; Selected for high relevance via seed_file: Seed file defines symbol agent_route; matched task keywords: agent, route; evidence mix: seed file x10, call graph x1
18. Continuation status is omitted_candidates_available; next follow-up is src/main.rs at candidate rank 2 because token_budget_exhausted; next_action=run_omitted_candidate_context_pack.
19. impact_analysis reports high risk across 8 impacted files with 4 suggested checks; after selected context is read, pre-edit impact check estimated 8 impacted files at high risk, including 7 call-related files, 3 dependency-related files, 50 call paths, and 0 dependency paths

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
  first selection rank, current reading step mirror contract,
  executable reading-plan reason, raw selection reason,
  token estimate, line reduction, route reason, continuation status,
  continuation next action, and omitted-candidate status.
- `impact_analysis` includes its route reason so the demo frames it as the
  pre-edit impact check after selected context is read.
- The evidence summary gives a compact copyable result for README videos or
  project introductions, including blind first-read baseline, routed first-read
  lines, avoided source lines, and read-less ratio.
- The talk track explains the same path a user should show in a recording.
- The agent policy matches the MCP client workflow.
- `CODEINSIGHT_DEMO_SAVE_JSON` can persist the raw `agent_route` payload for
  issue reports, README recordings, benchmark evidence, or client integration
  debugging.

Refresh this snapshot after material changes to indexing, project overview,
context packing, impact analysis, or demo wording.
