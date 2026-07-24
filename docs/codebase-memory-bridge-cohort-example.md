# codebase-memory Bridge Cohort Example

This is a maintainer-run example for the adjusted product direction:
codebase-memory-mcp provides local graph retrieval evidence, while CodeInsight
turns that evidence into an agent-facing first-read route, bounded context,
quality checks, and a verification plan.

The snapshot below was collected on 2026-07-24 against this repository after
refreshing the codebase-memory index in `fast` mode.

## Setup

Refresh the graph backend first:

```text
index_repository(
  repo_path="/Users/binlee/code/open-source/CodeInsight-mcp",
  name="CodeInsight-mcp",
  mode="fast"
)
```

The live index returned `2063` nodes and `8708` edges.

## Backend Queries

Use precise graph queries to collect advisory candidate files:

| Task | codebase-memory query | Backend top file |
| --- | --- | --- |
| `understand agent route backend evidence and first read routing` | `search_graph(name_pattern=".*agent_route.*", label="Function")` | `src/tools.rs` |
| `understand MCP tool dispatch and tool definitions` | `search_graph(name_pattern=".*tool_definitions.*|.*handle_request.*|.*call_tool.*", label="Function")` | `src/mcp.rs` |
| `understand semantic embedding provider configuration` | `search_graph(name_pattern=".*provider.*|.*Embedding.*", file_pattern="src/embedding.rs")` | `src/embedding.rs` |

Normalize those candidate files into the `backend_evidence` object consumed by
`agent_route`. The evidence is advisory: CodeInsight must preserve route-quality
warnings and verification steps instead of blindly trusting the backend order.

## CodeInsight Route Runs

Run `agent-route` once per task with the corresponding backend evidence:

```bash
codeinsight agent-route /Users/binlee/code/open-source/CodeInsight-mcp \
  --task "understand agent route backend evidence and first read routing" \
  --token-budget 6000 \
  --backend-evidence /tmp/agent-route-evidence.json \
  > /tmp/agent-route.json

codeinsight agent-route /Users/binlee/code/open-source/CodeInsight-mcp \
  --task "understand MCP tool dispatch and tool definitions" \
  --token-budget 6000 \
  --backend-evidence /tmp/mcp-dispatch-evidence.json \
  > /tmp/mcp-dispatch.json

codeinsight agent-route /Users/binlee/code/open-source/CodeInsight-mcp \
  --task "understand semantic embedding provider configuration" \
  --token-budget 6000 \
  --backend-evidence /tmp/semantic-provider-evidence.json \
  > /tmp/semantic-provider.json
```

Then generate bridge reports and the cohort summary:

```bash
scripts/codebase-memory-bridge-report.sh \
  --backend-evidence /tmp/agent-route-evidence.json \
  --agent-route-json /tmp/agent-route.json \
  --task "understand agent route backend evidence and first read routing" \
  --output-dir /tmp/codeinsight-codebase-memory-bridge/agent-route

scripts/codebase-memory-bridge-cohort-summary.sh \
  /tmp/codeinsight-codebase-memory-bridge/agent-route \
  /tmp/codeinsight-codebase-memory-bridge/mcp-dispatch \
  /tmp/codeinsight-codebase-memory-bridge/semantic-provider \
  --min-reports 3 \
  --check
```

## Snapshot

| Task | Backend top file | CodeInsight first file | Status | Route quality |
| --- | --- | --- | --- | --- |
| `understand agent route backend evidence and first read routing` | `src/tools.rs` | `src/tools.rs` | `pass` | `high 100/100` |
| `understand MCP tool dispatch and tool definitions` | `src/mcp.rs` | `src/mcp.rs` | `pass` | `high 100/100` |
| `understand semantic embedding provider configuration` | `src/embedding.rs` | `src/embedding.rs` | `pass` | `high 100/100` |

Aggregate result:

- Cohort status: `pass`
- Reports: `3/3`
- First-file top match rate: `100%`
- First-file candidate match rate: `100%`
- Selected backend candidate rate: `66.66%`
- Next action: `run_more_real_backend_tasks`

## Interpretation

This is not a claim that CodeInsight beats codebase-memory-mcp at graph
retrieval. It shows the narrower integration point is viable:

- codebase-memory can be the upstream candidate provider
- CodeInsight can preserve that evidence in `routing_decision.backend_evidence`
- `route_quality` can expose backend agreement as confidence evidence
- `route_quality.verification_steps` still tells the agent to treat backend
  evidence as advisory
- the cohort script can aggregate agreement and conflict rates across tasks

The next useful validation is to repeat this on non-CodeInsight repositories
and intentionally include at least one disagreement case, so the conflict review
path is tested with real backend output rather than only fixtures.
