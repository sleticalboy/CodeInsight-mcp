# Demo Script

Use this script for a two-minute walkthrough, README video, conference demo, or
open-source project introduction. The goal is to show CodeInsight as a
local-first context router for AI coding agents, not as a replacement for an
IDE, LSP, compiler, or hosted code search platform.

## Demo Goal

Show that an agent can start from a repository root and quickly get:

- a local index
- likely entrypoints
- a token-budgeted context pack
- next recommended tools
- an impact-analysis preview before edits

## Setup

From a CodeInsight checkout:

```bash
cargo build --locked --release
scripts/two-minute-demo.sh
```

Against another repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

When you need the raw route payload for an issue, README recording, benchmark
evidence, or client integration debugging:

```bash
CODEINSIGHT_DEMO_SAVE_JSON=/tmp/codeinsight-agent-route.json scripts/two-minute-demo.sh
```

For machine-readable CI validation, `scripts/two-minute-demo.sh` runs the
`agent-route` CLI command and renders the first-read metrics. Use
`scripts/agent-router-demo.sh` when you need lower-level metrics, reading
reasons, impact breakdown output, and CI-style assertions.

Use `scripts/framework-entrypoint-demo.sh` when you need a compact local proof
that framework-oriented first reads route matching tasks to Next.js app router,
Rails routes, Django urls, and C# startup files.

For a copyable example of the current repository output, see the
[demo output snapshot](demo-output.md).

Optional smoke validation:

```bash
scripts/mcp-stdio-smoke.sh
```

## Two-Minute Talk Track

### 0:00-0:20 - Problem

AI coding agents waste time and tokens on the first read of a repository. They
often start by guessing search terms, running broad file scans, opening too
many files, and still missing the actual entrypoint or change radius.

The core problem is not editing code. The core problem is routing the agent to
the right local context before it edits.

### 0:20-0:35 - Positioning

CodeInsight is a local-first MCP code context router. It indexes the repository
locally, summarizes structure, chooses a bounded context pack, and suggests the
next tool call.

It is intentionally not trying to be a compiler-grade analyzer or a replacement
for Sourcegraph. It is designed to make an agent read less code and make fewer
blind guesses.

### 0:35-1:10 - Run The Demo

Run:

```bash
scripts/two-minute-demo.sh
```

Point out the four stages:

1. `index_project`
2. `project_overview`
3. `context_pack`
4. `impact_analysis`

Expected shape:

```text
CodeInsight two-minute demo

Problem: AI agents waste the first read by scanning broad files and guessing entrypoints.
Promise: route the agent through agent_route before edits.

1. index_project
   indexed_files: 150
   symbols: 2137

2. project_overview
   entrypoints: 12
   recommended_next_tools: 5

3. context_pack
   selected_files: 1
   selected_ranges: 10
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
   routing_decision_read_less: 99.4%, 163.8x
   routing_decision_continuation: omitted_candidates_available
   routing_decision_impact_status: complete
   routing_decision_quality: high (100/100, 23 evidence signals)
   routing_decision_recommended_action: read_selected_context_then_use_continuation_if_needed
   first_next_action: inspect_seed_file
   first_reading_focus: Start with seed file context routing, first-read handoff, and read-less evidence.
   first_reading_question: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here?
   first_selection_rank: 1
   first_context_file: src/tools.rs
   first_reading_file: src/tools.rs
   reading_order_contract: true
   read_less_instruction_contract: true
   current_reading_step_contract: true
   suggested_tool_handoff_contract: true
   continuation_timing_contract: true
   total_lines: 86146
   selected_lines: 526
   source_lines_avoided: 85620
   line_reduction: 99.4%
   read_less_ratio: 163.8x
   continuation: omitted_candidates_available
   continuation_next_action: run_omitted_candidate_context_pack
   first_omitted_candidate: src/main.rs (candidate rank 2)
   first_omitted_reason: token_budget_exhausted
   first_omitted_next_action: run_omitted_candidate_context_pack
   route_reason: selected 1 files, 10 ranges, and 1 reading-plan steps within the token budget; read src/tools.rs first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; first omitted candidate src/main.rs (candidate rank 2, reason token_budget_exhausted) can be revisited via run_omitted_candidate_context_pack using context_pack after selected context; continuation run_omitted_candidate_context_pack

4. impact_analysis
   risk_level: high
   impacted_files: 8
   suggested_checks: 4
   route_reason: after selected context is read, pre-edit impact check estimated 8 impacted files at high risk, including 7 call-related files, 3 dependency-related files, 50 call paths, and 0 dependency paths

[Evidence summary]
Blind first-read baseline: 86146 source lines.
Routed first-read: 526 source lines across 1 files.
Read less: avoided 85620 source lines, 163.8x less text before follow-up tools.
Routing decision: seed=task_match:src/tools.rs, first_file=src/tools.rs, rank=1, tool=file_outline, continuation=omitted_candidates_available, impact=complete.
Route quality: high (100/100) from 23 evidence signals; next=read_selected_context_then_use_continuation_if_needed.
agent_route selected 526/86146 source lines (99.4% reduction) across 1 files.
First reading focus: Start with seed file context routing, first-read handoff, and read-less evidence.
First reading question: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here?
The first selected file is src/tools.rs; reading_plan starts at src/tools.rs as candidate rank 1.
Execution contract: reading_order=true, read_less_instruction=true, current_reading_step=true, suggested_tool_handoff=true, continuation_after_selected_context=true.
Selection evidence: Selected for high relevance via seed_file: Seed file defines symbol agent_route; matched task keywords: agent, route; Seed file defines symbol read_agent_route_backend_evidence; matched task keywords: agent, route; evidence mix: seed file x9, call graph x1
Continuation: status=omitted_candidates_available, next_action=run_omitted_candidate_context_pack.
Next follow-up candidate: src/main.rs at candidate rank 2; token_budget_exhausted; next_action=run_omitted_candidate_context_pack.
Read src/tools.rs before offering file_outline.
Before edits, impact_analysis reports high risk across 8 impacted files.

[Talk track]
1. agent_route ran index_project, project_overview, context_pack, and impact_analysis in one call.
2. project_overview found 12 entrypoints and 5 recommended next tools.
3. context_pack selected 1 files and 10 ranges, then produced 1 reading-plan steps.
4. execution_plan starts with read_selected_context, then use_current_reading_step_suggested_tool; this keeps suggested tools behind selected-context reading.
5. The first execution-plan suggested tool is file_outline; offer it only after the selected file has been read.
6. route_quality is high (100/100) from 23 evidence signals; recommended_action=read_selected_context_then_use_continuation_if_needed.
7. The first reading-plan focus is: Start with seed file context routing, first-read handoff, and read-less evidence.
8. The first reading-plan question is: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here?
9. The first reading-plan action is inspect_seed_file; the selected context avoided 85620 source lines (99.4%, 163.8x less text); selected 1 files, 10 ranges, and 1 reading-plan steps within the token budget; read src/tools.rs first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; first omitted candidate src/main.rs (candidate rank 2, reason token_budget_exhausted) can be revisited via run_omitted_candidate_context_pack using context_pack after selected context; continuation run_omitted_candidate_context_pack
10. Reading order contract is true; execution_plan[0].files follows reading_plan[] order.
11. Read-less instruction contract is true; execution_plan[0].instruction carries selected lines, baseline lines, avoided lines, and read-less ratio.
12. Current reading step contract is true; agent_route.current_reading_step mirrors reading_plan[0].
13. Suggested-tool handoff contract is true; execution_plan[1] points to the current reading step.
14. Continuation timing contract is true; continuation is only considered after selected context is read.
```

Exact numbers vary by repository and current source state. The important point
is that CodeInsight turns a repository into a small, ordered reading plan.

### 1:10-1:35 - Explain The Output

`project_overview` answers "where should the agent start?" It returns
entrypoint candidates, directory roles, summaries, and recommended next tools.

`context_pack` answers "what should fit into the model context now?" It returns
selected files, line ranges, excerpts, a reading plan, budget metadata, and a
continuation strategy if more context is needed.

`first_reading_focus` is the compact scan label for the first reading step. It
gives demo viewers a shorter cue than the full reading question while preserving
the same task intent.

`route_reason` turns the route into an executable explanation: it says which
file to read first, which action to take, which tool to use for deeper evidence,
and why `impact_analysis` is the pre-edit check after selected context is read.

`routing_decision` is the compact display row for the same first-read choice:
seed source/value, first file, rank, suggested tool, read-less evidence,
continuation state, and impact status. `selection_rank` and
`selection_reason` make the first-read choice auditable. They show that the
first file is not just a returned excerpt; it is the top candidate under the
current task and token budget.

`continuation_summary` and `omitted_candidates[]` tell the agent what to do
after the selected context is consumed. In a complete context pack, the demo
prints that no omitted follow-up is needed yet. In a tighter budget, it names
the next candidate and why it was omitted.

`line_reduction` shows the routing value. The agent does not need to read the
whole repository to start. It can begin with a bounded context pack and ask for
focused continuation only when necessary.

`impact_analysis` answers "what might I affect if I change this?" It gives a
local risk preview and suggested checks before edits.

### 1:35-1:50 - Evidence Cutaway

For a README video or project introduction, briefly point to the benchmark
reports after the live demo:

```text
Smoke benchmark: context_pack first for 4/4 repositories, 99.2% aggregate line reduction.
Large benchmark: context_pack first for 4/4 repositories, 99.3% aggregate line reduction.
```

The `Key Results` section in each report is the stable evidence slide. It
summarizes routing, context compression, token budget, indexing, guardrails,
and truncation status without asking viewers to inspect the full details table.

### 1:50-1:58 - MCP Agent Flow

In a real MCP client, the agent policy is:

```text
1. Call agent_route with root, task, and token_budget for the default first read.
2. Read context_pack.files in reading_plan order.
3. Use continuation_summary only after selected context is consumed.
4. Run focused follow-up tools only when needed.
```

This is the path that turns CodeInsight from a CLI demo into an agent workflow.
Clients that need custom routing can still call `index_project`,
`project_overview`, `context_pack`, and `impact_analysis` directly.

### 1:58-2:00 - Close

The product promise is simple: keep code local, route the agent to the right
context, and reduce blind reading before edits.

## Demo Checklist

Before recording or presenting:

- Run `cargo build --locked --release`.
- Run `scripts/two-minute-demo.sh`.
- Run `scripts/agent-router-demo.sh` when you need assertion-oriented metrics,
  reading reasons, and impact breakdown output.
- Run `scripts/framework-entrypoint-demo.sh` when framework entrypoint routing
  is part of the demo, README update, or regression check.
- Compare with [Demo output snapshot](demo-output.md) when preparing README
  videos, release notes, or project introductions.
- Confirm `indexed_files` is greater than zero.
- Confirm `recommended_next_tools` is greater than zero.
- Confirm `context_pack` reports selected files, `reading_plan_steps`, and
  `line_reduction`; the demo fails fast if selected files, reading-plan steps,
  or the first next action are missing.
- Confirm `impact_analysis` reports risk or suggested checks.
- Keep the benchmark `Key Results` available if the audience asks for evidence
  beyond the live demo.
- Keep [Known limitations](known-limitations.md) available if asked about
  static-analysis precision.

## If The Demo Is Run On Another Repository

Use:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo \
CODEINSIGHT_DEMO_TASK="understand the main application entrypoint" \
scripts/two-minute-demo.sh
```

If the repository has no obvious source entrypoint, explain that as product
signal: `project_overview` still recommends a fallback tool path, often
`context_pack`, `dependency_graph`, `callers`, or `config_status`.

## Suggested Follow-Up Links

- [Quickstart](quickstart.md)
- [Adoption checklist](adoption-checklist.md)
- [Client workflow](client-workflow.md)
- [MCP client configuration](mcp-client-config.md)
- [Smoke benchmark](benchmark-v0.1.md)
- [Large repository benchmark](benchmark-large.md)
