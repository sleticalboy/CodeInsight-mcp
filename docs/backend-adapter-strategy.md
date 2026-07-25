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
selection. When the backend agrees on the first file, `route_quality` records
that as confidence evidence. When the backend prefers a different first file,
`route_quality.warnings` names the mismatch and `recommended_action` changes to
`compare_backend_route_before_edits` before the agent edits code.

The first bridge prototype is script-level and runtime-agnostic:

```bash
scripts/codebase-memory-backend-evidence.sh \
  --root /path/to/repo \
  --search-graph-json /tmp/codebase-memory-search-graph.json \
  --search-code-json /tmp/codebase-memory-search-code.json \
  --architecture-json /tmp/codebase-memory-architecture.json \
  --output /tmp/codeinsight-backend-evidence.json
```

This script expects exported JSON responses from `search_graph`, `search_code`,
and `get_architecture`, normalizes candidate file paths relative to `--root`,
and emits the `backend_evidence` object consumed by CLI `agent-route` and MCP
`agent_route`. It deliberately does not call codebase-memory-mcp itself; Codex,
Claude Code, Cursor, CI jobs, or manual benchmark runs can supply those exported
tool responses.

After running `agent-route`, summarize backend/local agreement:

```bash
scripts/codebase-memory-bridge-report.sh \
  --backend-evidence /tmp/codeinsight-backend-evidence.json \
  --agent-route-json /tmp/codeinsight-agent-route.json \
  --task "understand agent context routing" \
  --output-dir /tmp/codeinsight-codebase-memory-bridge
```

The report records whether the local first file matches the backend top file,
whether selected context covers backend candidates, whether backend evidence is
visible in `route_quality`, and whether the advisory verification step survived
into the agent-facing route.

For multi-task evidence, aggregate several bridge reports:

```bash
scripts/codebase-memory-bridge-cohort-summary.sh \
  /tmp/codeinsight-codebase-memory-bridge/task-1 \
  /tmp/codeinsight-codebase-memory-bridge/task-2 \
  --min-reports 2 \
  --check
```

If raw evidence and route JSON pairs are already collected, generate all
per-task reports and the aggregate cohort with one manifest:

```bash
scripts/codebase-memory-bridge-cohort-report.sh \
  --manifest /tmp/codeinsight-codebase-memory-bridge.tsv \
  --output-dir /tmp/codeinsight-codebase-memory-bridge-cohort \
  --min-reports 3 \
  --check
```

Manifest rows use
`slug<TAB>task<TAB>backend_evidence_json<TAB>agent_route_json`.

The native backend can keep using CodeInsight's current index. A future
codebase-memory adapter can call `search_graph`, `search_code`, `trace_path`,
or `get_architecture`, then hand candidates to CodeInsight's existing
`context_pack` and `agent_route` quality logic.

A maintainer-run 3-task bridge example is checked in at
[codebase-memory bridge cohort example](codebase-memory-bridge-cohort-example.md).
It records a live codebase-memory `fast` index over this repository, three
precise backend queries, three `agent-route --backend-evidence` runs, and an
aggregate bridge cohort with `3/3` pass reports and `100%` first-file top match
rate.

## Near-Term MVP

Do not add a hard runtime dependency on codebase-memory-mcp yet. The MVP step is
to make the route evidence format backend-ready:

- every matrix row exposes `route_quality_decision_summary`
- every matrix row exposes `route_quality_confidence_factors`
- every matrix row exposes `route_quality_verification_steps`
- every public route snapshot shows route quality beside first-file results
- `agent_route` accepts optional backend evidence and reflects it in
  `routing_decision.route_quality`
- backend/local first-file conflicts change `route_quality.recommended_action`
  to `compare_backend_route_before_edits`
- `scripts/codebase-memory-backend-evidence.sh` normalizes exported
  codebase-memory results into `backend_evidence`
- `scripts/codebase-memory-bridge-report.sh` summarizes real backend/local
  agreement artifacts after `agent-route`
- `scripts/codebase-memory-bridge-cohort-summary.sh` aggregates multiple
  agreement reports into first-file match rates and conflict counts
- `scripts/codebase-memory-bridge-cohort-report.sh` batch-generates per-task
  agreement reports and the aggregate cohort from a TSV manifest
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
