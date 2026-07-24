# Backend Adapter Strategy

CodeInsight should not compete with codebase-memory-mcp as a generic local code
knowledge graph. If another local backend finds task entrypoints faster, more
accurately, and with less token cost, CodeInsight should consume that signal
instead of rebuilding it.

The product boundary is:

```text
graph/backend evidence -> agent_route -> context_pack -> verification plan
```

## Product Role

CodeInsight owns the agent workflow layer:

- normalize backend evidence into a first-read route
- select bounded files and ranges under a token budget
- explain why the route is trustworthy or blocked
- show confidence factors, warnings, and verification steps
- suggest the next local tool after the selected context is read
- preview impact and checks before edits

Backends own retrieval:

- symbol and file discovery
- graph search
- callers/callees
- dependency and architecture facts
- optional semantic search

## Adapter Shape

The initial adapter boundary should be small and task-oriented:

```text
resolve_task_candidates(root, task, seeds, limit)
  -> candidates[]

candidate:
  file
  symbol?
  source
  score?
  reason
  evidence[]
```

The current MVP contract accepts advisory backend evidence directly in
`agent_route`:

```json
{
  "provider": "codebase-memory-mcp",
  "candidate_files": ["src/main.ts"],
  "evidence_sources": ["entry_points", "call_graph"],
  "evidence_count": 7,
  "latency_ms": 42,
  "confidence": 0.91,
  "notes": ["external backend agreed with the local first-read route"]
}
```

CLI usage:

```bash
codeinsight agent-route /path/to/repo \
  --task "understand app entrypoint flow" \
  --backend-evidence /tmp/codeinsight-backend-evidence.json
```

MCP usage passes the same object as `backend_evidence` in the `agent_route`
arguments. CodeInsight uses this evidence to adjust route confidence, warnings,
and verification steps. It does not blindly override the local `context_pack`
selection; conflicting backend candidates are surfaced as warnings.

The native backend can keep using CodeInsight's current index. A future
codebase-memory adapter can call `search_graph`, `search_code`, `trace_path`,
or `get_architecture`, then hand candidates to CodeInsight's existing
`context_pack` and `agent_route` quality logic.

## Near-Term MVP

Do not add a hard runtime dependency on codebase-memory-mcp yet. The MVP step is
to make the route evidence format backend-ready:

- every matrix row exposes `route_quality_decision_summary`
- every matrix row exposes `route_quality_confidence_factors`
- every matrix row exposes `route_quality_verification_steps`
- every public route snapshot shows route quality beside first-file results
- `agent_route` accepts optional backend evidence and reflects it in
  `routing_decision.route_quality`
- comparison docs describe codebase-memory as a possible provider, not just a
  competitor

This keeps CodeInsight useful even when native retrieval is weaker: the product
can become the agent-facing route, budget, and verification layer over better
retrieval backends.

## Non-Goals

- no attempt to match codebase-memory-mcp language count in the MVP
- no Cypher-compatible graph query surface in CodeInsight MVP
- no watcher, graph UI, ADR storage, or team graph snapshot work before the
  workflow layer is validated
- no claim that CodeInsight's native backend is more accurate than specialized
  graph/LSP backends
