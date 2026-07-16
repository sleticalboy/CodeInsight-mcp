# Two-Minute Demo Output Snapshot

This snapshot captures the expected user-facing shape of
`scripts/two-minute-demo.sh` when it is run from the CodeInsight repository.
Use it as a copyable reference for README videos, project introductions, and
release checks.

The numbers below are from the current repository state on 2026-07-16. Counts
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
Promise: route the agent through project_overview, context_pack, and impact_analysis before edits.

[Live run]
building release binary...
    Finished `release` profile [optimized] target(s) in 0.33s
CodeInsight agent context router demo
root: <repo>/CodeInsight-mcp
task: understand agent context routing
token_budget: 6000

1. index_project
   indexed_files: 23
   symbols: 911
   duration_ms: 309
   errors: 0

2. project_overview
   total_lines: 27331
   entrypoints: 7
   first_entrypoint: src/main.rs
   recommended_next_tools: 4

3. context_pack
   selected_files: 10
   selected_ranges: 11
   reading_plan_steps: 8
   first_next_action: inspect_seed_file
   selected_lines: 428
   line_reduction: 98.4%
   estimated_tokens: 4276
   continuation: complete
   first_context_file: src/main.rs

4. impact_analysis
   seed_file: src/main.rs
   risk_level: high
   impacted_files: 11
   paths: 0
   suggested_checks: 3

Run against another repository:
  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/agent-router-demo.sh

[Talk track]
1. project_overview found 7 entrypoints and 4 recommended next tools.
2. context_pack selected 10 files and 11 ranges, then produced 8 reading-plan steps.
3. The first action is inspect_seed_file; the selected context reduced source reading by 98.4%.
4. Continuation status is complete, so the agent knows whether to ask for a focused follow-up.
5. impact_analysis reports high risk across 11 impacted files with 3 suggested checks.

[Agent policy]
Call index_project, then project_overview, then context_pack with a token budget.
Read context_pack.files in reading_plan order, use continuation_summary only after that, and run impact_analysis before edits.

Run this walkthrough against another repository:
  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

## What To Check

- The path starts with `index_project`, then `project_overview`, then
  `context_pack`, then `impact_analysis`.
- `context_pack` includes selected files, selected ranges, reading-plan steps,
  first next action, token estimate, line reduction, and continuation status.
- The talk track explains the same path a user should show in a recording.
- The agent policy matches the MCP client workflow.

Refresh this snapshot after material changes to indexing, project overview,
context packing, impact analysis, or demo wording.
