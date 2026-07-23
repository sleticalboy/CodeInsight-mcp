# Competitive Analysis: codebase-memory-mcp

This note compares CodeInsight with
[codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp). It is a
positioning and evaluation note, not a claim that CodeInsight has reproduced the
other project's published benchmark results.

## Short Answer

The tools are adjacent, but they should not be positioned as direct substitutes.

codebase-memory-mcp is a local code knowledge-graph backend. Its public README
positions it around broad graph intelligence: many languages, fast indexing,
Hybrid LSP, persistent graph queries, semantic search, dead-code detection,
cross-repository links, ADR storage, graph visualization, watchers, and broad
agent installation support.

CodeInsight is an AI-agent first-read router. Its strongest product surface is
not the size of the graph. It is the route contract:

```text
agent_route -> context_pack -> reading_plan -> suggested_tool -> impact_analysis
```

That contract tells an agent which files and ranges to read first, why those
ranges were selected, how much source was avoided, what follow-up tool should be
used, and what impact check should run before edits.

## Current Strength Comparison

| Dimension | Stronger Today | Why |
| --- | --- | --- |
| Generic code knowledge graph | codebase-memory-mcp | Public positioning centers on persistent graph storage, graph query, architecture, cross-service, and cross-repo edges. |
| Language coverage | codebase-memory-mcp | Its README claims 158 vendored tree-sitter grammars and Hybrid LSP support for selected languages. |
| Graph query and visualization | codebase-memory-mcp | Its README advertises Cypher-like queries and an optional graph UI. |
| Distribution and install surface | codebase-memory-mcp | Its README and release metadata show a mature multi-platform static-binary story and broad agent install support. |
| Agent first-read workflow | CodeInsight | CodeInsight has a narrower contract for `agent_route`, selected context, reading plans, continuation, and pre-edit impact checks. |
| Token-budgeted context selection | CodeInsight | `context_pack` exposes selected ranges, read-less metrics, omitted candidates, and continuation guidance. |
| Public route-quality evidence | CodeInsight | Checked-in reports focus on first-file routing quality and source-line reduction for agent tasks. |

## Positioning Decision

CodeInsight should not claim to be a local Sourcegraph replacement, a universal
knowledge graph, or a broader graph database than codebase-memory-mcp. That
would force CodeInsight into a feature-count contest where it is currently
weaker and where the roadmap would drift away from the MVP goal.

The stronger positioning is:

> Local-first first-read router for AI coding agents.

In practical terms, CodeInsight should optimize for:

- first key file selection
- bounded first-read context under a token budget
- reading-plan explanations
- actionable next-tool handoff
- omitted-candidate continuation
- pre-edit impact preview
- copyable route-quality evidence

It should avoid making broad claims about compiler-grade analysis, universal
semantic understanding, or complete graph coverage.

## Evaluation Method

Use the same repositories and the same task prompts for both tools. Compare
outcomes from the agent workflow perspective rather than feature breadth.

Primary metrics:

- first selected file
- whether the first selected file matches the expected task owner
- selected source lines before follow-up tools
- source lines avoided versus blind first-read baseline
- token estimate for selected context
- whether a reading plan is present
- whether a follow-up tool is suggested
- whether a pre-edit impact check is available

Secondary metrics:

- index time
- tool calls needed to reach the first useful context
- whether the result explains why each file was selected
- whether omitted context has a bounded continuation path

Non-goals for this comparison:

- proving generic graph-query superiority
- proving compiler-grade reference precision
- reproducing every published benchmark from either project
- ranking package installation maturity

## Minimal Local Check

Run the deterministic no-network comparison scaffold:

```bash
scripts/competitive-routing-smoke.sh
```

The smoke does not require codebase-memory-mcp to be installed. It verifies that
CodeInsight can produce the local first-read metrics and JSON shape needed for a
fair side-by-side comparison. If a competitor result export exists later, use
the same task and repository fields and add it to the report instead of changing
the success criteria.

## Strategic Implication

The next CodeInsight work should improve the agent route contract rather than
trying to clone every graph backend feature:

- make first-file routing harder to fool
- improve route explanations and confidence signals
- keep benchmark evidence reproducible
- expose optional provider boundaries cleanly
- consider third-party graph backends as inputs later

That leaves room for codebase-memory-mcp to be treated as a possible future
graph provider instead of only as a competitor.
