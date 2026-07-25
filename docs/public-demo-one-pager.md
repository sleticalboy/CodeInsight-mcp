# Public Demo One-Pager

Use this one-pager when preparing a README recording, launch post, demo call, or
open-source project introduction. It keeps the public story focused:
CodeInsight is a local-first MCP code context router for AI coding agents.

## One-Sentence Positioning

CodeInsight helps an AI coding agent start a repository task by routing it to a
small, ordered first-read context, an executable next tool, and a pre-edit
impact preview before it opens files blindly.

## Show This Workflow

```text
agent_route -> selected context -> executable suggested_tool -> impact check
```

The demo should make four points visible:

- The agent starts from a repository root and task, not hand-picked files.
- `context_pack` selects bounded files/ranges under a token budget.
- `routing_decision` gives one compact audit row for the first seed, first file,
  rank, next tool, read-less evidence, continuation state, and impact status.
- `impact_analysis` runs before edits, so the first read ends with a change-risk
  preview instead of only navigation metadata.

## Run The Demo

From this repository:

```bash
scripts/two-minute-demo.sh
```

Against another repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

Save the raw route payload for an issue, recording, or client integration note:

```bash
CODEINSIGHT_DEMO_SAVE_JSON=/tmp/codeinsight-agent-route.json scripts/two-minute-demo.sh
```

## What To Point At

Expected output shape:

```text
3. context_pack
   routing_decision_seed_strategy: auto_task_match
   routing_decision_first_seed: task_match:src/tools.rs
   routing_decision_first_file: src/tools.rs
   routing_decision_first_selection_rank: 1
   routing_decision_suggested_tool: file_outline
   routing_decision_read_less: 99.4%, 153.9x
   routing_decision_continuation: omitted_candidates_available
   routing_decision_impact_status: complete
   routing_decision_quality: high (100/100, 22 evidence signals)
   routing_decision_recommended_action: read_selected_context_then_use_continuation_if_needed
   first_reading_focus: Start with seed file context routing, first-read handoff, and read-less evidence.
   first_reading_question: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here?

4. impact_analysis
   risk_level: high
   impacted_files: 16
   suggested_checks: 4

[Evidence summary]
Routing decision: seed=task_match:src/tools.rs, first_file=src/tools.rs, rank=1, tool=file_outline, continuation=omitted_candidates_available, impact=complete.
Route quality: high (100/100) from 22 evidence signals; next=read_selected_context_then_use_continuation_if_needed.
Read src/tools.rs before offering file_outline.
Before edits, impact_analysis reports high risk across 16 impacted files.
```

Exact numbers vary by repository and current source state. The stable signal is
the route shape: first seed, first file, reading focus/question, read-less
evidence, suggested tool handoff, continuation status, and pre-edit impact.

## Evidence To Cite

- Two-minute demo snapshot: [demo-output.md](demo-output.md).
- Full demo talk track: [demo-script.md](demo-script.md).
- Public route-quality matrix:
  [public-task-routing-matrix.md](public-task-routing-matrix.md).
- Public route-quality JSON:
  [public-task-routing-matrix-summary.json](public-task-routing-matrix-summary.json).
- Benchmark methodology: [benchmark-methodology.md](benchmark-methodology.md).
- MVP public gate: [mvp-public-readiness.md](mvp-public-readiness.md).

Current public route-quality snapshot:

- Express, FastAPI, Flask, Gin, Requests, Streamlit, and Wouter pass `92/92` expected
  first-file checks.
- The public matrix selects `41,664` of `7,142,226` task source lines for a
  `99.41%` aggregate first-read line reduction.
- A heavyweight Django probe passes `3/3` expected first-file checks and shows a
  `99.87%` aggregate first-read line reduction in the latest local verification.

Treat these numbers as first-read routing and token-discipline evidence, not as
runtime performance claims or proof that unselected code is irrelevant.

## Talk Track

1. "The agent asks one local route question before opening files broadly."
2. "`agent_route` runs indexing, overview, context packing, and impact preview in
   one first-read workflow."
3. "`routing_decision` is the compact row a client can display: seed, first file,
   rank, tool, read-less evidence, continuation, impact."
4. "The selected context is not the whole repository. The read-less metrics show
   how much source text the agent avoided before focused follow-up."
5. "The suggested tool is behind selected-context reading, so the agent reads the
   chosen file before asking for deeper local evidence."
6. "Before edits, `impact_analysis` gives a local risk preview and suggested
   checks."

## Guardrails

Say:

- Local-first MCP code context router for AI coding agents.
- First-read routing, reading plans, token-budgeted context, and impact preview.
- Best-effort local static signals that help the agent choose where to start.

Do not say:

- IDE, LSP, compiler, or Sourcegraph replacement.
- Compiler-grade static analysis.
- Semantic search quality by default without a configured embedding provider.
