# CodeInsight MCP Server

CodeInsight MCP Server is a local-first first-read router for AI coding agents.
It turns "scan the repository and guess what matters" into a bounded local route:
index the project, choose the first context pack, offer the next focused tool,
and require a focused impact check before edits.

The product is intentionally narrow. It is built for the first-read problem:
before an agent edits a repository, it needs to know which files matter, what
entrypoints to inspect, what context fits the token budget, and what may be
impacted by a change. CodeInsight keeps that loop local and exposes it through
CLI and MCP tools.

The primary agent loop starts with one call:

```text
agent_first_read -> selected context -> executable suggested_tool -> impact check
```

That route gives the agent:

- selected files and line ranges instead of a broad repository scan
- `reading_plan[].focus` as the compact scan label for each selected file
- `reading_plan[].question` as the local checklist for the current file
- `reading_plan[].reason` instructions that explain what to read first and why
- `context_pack.read_less` metrics that show first-read source-line reduction
- `execution_plan[]` actions that keep focused follow-up tools behind the
  selected-context read
- an executable `suggested_tool` such as `file_outline` for deeper local
  evidence when needed
- a deferred `impact_analysis` call that remains required before edits

CodeInsight is not trying to replace an IDE, LSP, compiler, or Sourcegraph. Its
job is to help AI agents read less code, pick better context, and stop guessing
which file to open next.

Use CodeInsight when the agent needs a local reading route. Keep using the IDE,
LSP, compiler, test runner, and language-specific tools for precise diagnostics,
type checks, and final verification.

Current public routing snapshot: pinned Express, FastAPI, Flask, Gin, Requests,
Streamlit, and Wouter route-quality expectations pass `92/92`, selecting
41,664 of 7,142,226 task source lines for a `99.41%` aggregate first-read line reduction,
with each route carrying a first suggested tool such as `file_outline`. See the
checked-in [Markdown snapshot](docs/public-task-routing-matrix.md) and
[JSON summary](docs/public-task-routing-matrix-summary.json). A heavyweight
manual Django probe covers URL resolver routing, request/response lifecycle,
and middleware behavior with `3/3` expected first-file checks and a `99.87%`
aggregate first-read line reduction. Reproduce the default snapshots:

```bash
scripts/update-public-task-routing-matrix.sh --check
```

When intentionally refreshing public evidence, add
`--min-route-quality-score 70` to fail stale or low-confidence route-quality
output before publishing the regenerated snapshot.

For a deterministic no-network check of the same snapshot machinery:

```bash
scripts/update-public-task-routing-matrix-smoke.sh
```

Best-fit tasks:

- understand a new repository before opening many files
- plan an edit from a broad request such as "change the auth flow"
- collect the first files, reading questions, and follow-up tools for an agent
- preview likely impact before touching code

Do not treat CodeInsight output as compiler-grade proof. It is a local context
router: it narrows the first read, explains why each file was selected, and
hands the agent to precise local tools when the selected context is not enough.

## Fast Path

1. Install the binary:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | sh
   ```

2. Add the local MCP server:

   ```json
   {
     "mcpServers": {
       "codeinsight": {
         "command": "codeinsight",
         "args": ["serve", "--transport", "stdio"]
       }
     }
   }
   ```

   Codex, Claude Code, Cursor, and generic JSON examples are in
   [MCP client configuration](docs/mcp-client-config.md).

3. Tell the agent to start broad repository tasks with:

   ```text
   Call agent_first_read with root, task, and token_budget 6000 before reading files directly.
   It returns compact structuredContent, defers impact analysis, and caps the structured route at 8000 estimated tokens by default.
   Run the suggested impact_analysis before editing.
   If the task names an indexed file path such as src/auth.ts, pass it in task; CodeInsight can auto-seed that path without --file.
   If continuation_summary.status is blocked_no_seed, ask for a seed file or symbol instead of broad-reading.
   If it is blocked_invalid_seed or blocked_no_context, ask for an existing or matching seed and retry.
   Read selected files in reading_plan order and use selection_rank as the audit trail.
   Use reading_plan.focus as the compact scan label for the selected file.
   Treat reading_plan.question as the local checklist for the selected file.
   Use context_pack.read_less only as first-read reduction evidence.
   Use continuation_summary only after selected context is consumed.
   Follow agent_first_read.execution_plan[] in order.
   Use agent_first_read.routing_decision for a compact display of the first seed, first file, read-less metrics, continuation, and impact status.
   Use advanced agent_route only when another local graph backend produced advisory evidence or a synchronous impact preview is required.
   ```

   The first MCP `tools/call` payload is shown in
   [First Agent Route Call](docs/mcp-client-config.md#first-agent-route-call).

   `agent_first_read` can also invoke an installed `codebase-memory-mcp`
   binary before local routing. Backend failures fall back to the standalone
   local route by default and are reported in `backend_status`; set
   `backend.on_failure` to `error` when strict backend availability is required.

4. Pick the validation that matches your adoption stage:

   | Stage | Command | Use When |
   | --- | --- | --- |
   | First look | `scripts/two-minute-demo.sh` | You want a visible `agent_route -> context_pack -> impact_analysis` walkthrough with an `[Evidence summary]`. |
   | Framework entrypoints | `scripts/framework-entrypoint-demo.sh` | You want local proof that Next.js, Rails, Django, and C# web entrypoints can be detected and routed as first context for matching tasks. |
   | Task routing matrix | `scripts/task-routing-matrix.sh /path/to/repo --expect-file ./route-expectations.tsv --min-route-quality-score 80` | You want a multi-task first-read matrix showing first selected file, seed strategy, line reduction, token estimate, impact preview, optional explicit `--file` / `--symbol` seeds, expected-file gates, and an optional route-quality score gate for one repository. |
   | Public route matrix | `scripts/public-task-routing-matrix.sh --min-route-quality-score 70` | You want one aggregate route-quality summary across pinned checked-in Express, FastAPI, Flask, Gin, Requests, Streamlit, and Wouter expectation files, with an optional score gate for public evidence. Add `--case django` for the heavier manual Django route-quality probe. See the checked-in [public routing snapshot](docs/public-task-routing-matrix.md). |
   | MCP wiring | `CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/mcp-first-call-smoke.sh` | You want a compact JSON proof that stdio MCP accepts `agent_route`, returns the first context file, follows `reading_plan[]`, exposes route quality, read-less metrics, selection rank, and continuation evidence, runs the current step's suggested tool, includes impact suggested checks, and returns structured blocked routes with recovery actions for empty repositories, unusable explicit seeds, and unindexed task-path seeds. |
   | Installed adoption | `CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh` | You want the installed binary to pass CLI `agent-route`, MCP `agent_first_read`, deferred `impact_analysis`, and advanced `agent_route` against a temporary project. |
   | Local evidence | `scripts/adoption-evidence.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-evidence --print-snippet --issue-template` | You want one folder with local first-read evidence, raw route JSON, MCP first-call JSON, aggregate Markdown/JSON summaries, a copyable terminal snippet, and a ready-to-file issue template. |
   | External Beta trial | `scripts/external-beta-trial.sh /path/to/repo --output-dir /tmp/codeinsight-external-beta-trial` | You want a non-maintainer trial pack with an issue body, redaction checklist, maintainer triage note, and the underlying adoption evidence artifacts. |
   | External Beta cohort | `scripts/external-beta-cohort-summary.sh /tmp/beta-1 /tmp/beta-2 /tmp/beta-3 --min-route-quality-score 70 --check` | You want to aggregate at least three External Beta reports, gate low-confidence routes, count outcomes, and pick the next fix priority. |
   | External Beta fix queue | `scripts/external-beta-fix-queue.sh /tmp/codeinsight-external-beta-cohort.json --check` | You want to turn cohort outcomes into maintainer work items ordered by feedback priority. |
   | External Beta handoff | `scripts/external-beta-cohort-report.sh /tmp/beta-1 /tmp/beta-2 /tmp/beta-3 --output-dir /tmp/codeinsight-external-beta-handoff --check --print-snippet` | You want one folder with the cohort summary, route-quality gate, fix queue, README, machine-readable manifest, a ready-to-file issue body, and copyable public Beta handoff snippet. |
   | Adoption comparison | `scripts/adoption-comparison.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-comparison` | You want a shareable blind-read vs routed-first-read comparison showing source lines avoided, read-less ratio, seed strategy, optional explicit `--file` / `--symbol` seeds, optional task-critical `--expected-file` coverage, first reading focus/question, selection rank, and continuation next action. |
   | Backend evidence bridge | `scripts/codebase-memory-backend-evidence-smoke.sh` | You want proof that exported codebase-memory `get_code_snippet`, `search_graph`, `search_code`, `query_graph`, `trace_path`, and `get_architecture` JSON can become `agent_route --backend-evidence` and appear in route quality. |
   | Backend agreement report | `scripts/codebase-memory-bridge-report.sh --backend-evidence /tmp/codeinsight-backend-evidence.json --agent-route-json /tmp/codeinsight-agent-route.json --output-dir /tmp/codeinsight-codebase-memory-bridge` | You want a shareable backend/local agreement summary, including the agent-facing route action, after running `agent_route` with advisory codebase-memory evidence. |
   | Backend cohort summary | `scripts/codebase-memory-bridge-cohort-summary.sh /tmp/bridge-task-1 /tmp/bridge-task-2 --check` | You want aggregate first-file agreement and conflict evidence across multiple backend/local bridge reports. |
   | Backend cohort report | `scripts/codebase-memory-bridge-cohort-report.sh --manifest /tmp/bridge.tsv --output-dir /tmp/bridge-cohort --min-reports 3 --check` | You have multiple `backend_evidence` and raw `agent-route` JSON pairs and want reports plus the cohort summary in one command. |
   | Handoff report | `scripts/adoption-report.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-report --print-snippet` | You want a tar.gz report containing the evidence summaries, issue template, raw JSON, and diagnostic logs for upload or handoff. |

## Two-Minute Demo

Run the user-facing two-minute demo against this repository:

```bash
scripts/two-minute-demo.sh
```

Run it against another repository:

```bash
CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh
```

Save the raw `agent_route` JSON for issue reports, recordings, benchmark
evidence, or client integration debugging:

```bash
CODEINSIGHT_DEMO_SAVE_JSON=/tmp/codeinsight-agent-route.json scripts/two-minute-demo.sh
```

Generate a copyable Markdown evidence summary for your own repository:

```bash
scripts/adoption-evidence.sh /path/to/repo \
  --output-dir /tmp/codeinsight-adoption-evidence \
  --print-snippet \
  --issue-template
```

Package the same evidence into a handoff archive:

```bash
scripts/adoption-report.sh /path/to/repo \
  --output-dir /tmp/codeinsight-adoption-report \
  --print-snippet
```

Generate only the local first-read evidence files:

```bash
scripts/local-repo-evidence.sh /path/to/repo \
  --output /tmp/codeinsight-local-evidence.md \
  --json /tmp/codeinsight-agent-route.json \
  --summary-json /tmp/codeinsight-local-evidence.json
```

When comparing CodeInsight with a stronger local code graph, keep CodeInsight in
the agent workflow layer. Convert exported codebase-memory responses into
CodeInsight evidence. The bridge accepts raw payloads as well as exported MCP
`structuredContent` and JSON-RPC `result.content[].text` wrappers. When a
response contains multiple text blocks, the first valid JSON block is used:
For `search_graph`, the bridge uses non-empty semantic results before keyword
results and falls back to keyword results when the semantic array is empty.

```bash
scripts/codebase-memory-backend-evidence.sh \
  --root /path/to/repo \
  --search-graph-json /tmp/search-graph.json \
  --search-code-json /tmp/search-code.json \
  --architecture-json /tmp/architecture.json \
  --output /tmp/codeinsight-backend-evidence.json
```

Then pass that advisory evidence to the CLI:

```bash
codeinsight agent-route /path/to/repo \
  --task "understand app entrypoint flow" \
  --backend-evidence /tmp/codeinsight-backend-evidence.json
```

MCP callers can skip the file conversion step and pass a raw tool response
object or an ordered array of paginated response objects directly. The 64-item
budget applies across all pages for each tool after file-level candidate
deduplication; repeated symbols from the same file do not consume extra slots.
`agent_route` extracts bounded evidence and omits the raw payloads from its
response. A final page with `has_more: true` is reported as incomplete even
when the backend omits `total`:

```json
{
  "backend_evidence": {
    "provider": "codebase-memory-mcp",
    "tool_results": {
      "search_graph": {
        "results": [
          { "file_path": "src/auth.ts", "name": "AuthService" }
        ]
      }
    }
  }
}
```

Raw `get_code_snippet`, `search_graph`, `search_code`, `query_graph`,
`trace_path`, and `get_architecture` responses are accepted. Exact snippet
metadata is ranked before broader search results, while its `source` body is
discarded. Exported candidate-producing responses may also be ordered page
arrays; wrappers are unwrapped per page and latency is aggregated. The bridge
accepts at most 16 pages and 4 nested `result` wrappers, rejects paths outside
the project root, and spends each tool's 64-item budget only on unique candidate
files. For
`search_graph`, non-empty semantic `semantic_results` take
priority; an empty semantic array falls back to keyword `results`, and both use
the same per-tool budget. For `search_code`, structured `results`, file-only
`files`, and grep-style `raw_matches` are all converted into ranked file
candidates. For `query_graph`, return a
`file_path` or `file` column plus an optional `name` or `qualified_name` column;
CodeInsight converts the tabular `columns` and `rows` response into candidates.
For `trace_path`, CodeInsight resolves the traced subject, `callers`, and
`callees` back to files in its refreshed local index before routing.

To use backend-ranked candidates as bounded-context seeds, opt in explicitly:

```bash
codeinsight agent-route /path/to/repo \
  --task "understand app entrypoint flow" \
  --backend-evidence /tmp/codeinsight-backend-evidence.json \
  --prefer-backend-context
```

The MCP equivalent is `backend_evidence.prefer_for_context: true`. Candidates
retain backend order, while the token budget decides how many enter the reading
plan. `backend_route_agreement.selected_context_files` reports the candidates
actually retained. `backend_route_agreement.candidate_dispositions` explains
each candidate with its original rank, context status/reason, symbol status, and
an executable next action for reading, continuation, fallback, or evidence refresh.
When a valid backend candidate remains unselected,
`backend_route_agreement.next_candidate_continuation` exposes the highest-ranked
candidate plus a directly callable `context_pack` request. The same request is
promoted into `execution_plan` after the selected-context read, replacing the
generic continuation step without increasing plan length.
`routing_decision.continuation_source` identifies whether the effective step
comes from `backend_route_agreement` or the local `context_pack`; its status and
next action mirror the continuation that the execution plan will actually use.
Candidate symbols are checked against the local index; stale symbols are dropped
while their valid file candidates remain available.
Explicit `--file` or `--symbol` seeds still take priority, so a caller-selected
target is never replaced by backend ranking.

If the backend and local route agree on the first file, `route_quality` records
that agreement as confidence evidence and
`routing_decision.backend_route_agreement.status` is `agree`. If the backend
prefers a different first file, `backend_route_agreement.status` is `conflict`,
`route_quality.warnings` names the conflict, and
`route_quality.recommended_action` becomes `compare_backend_route_before_edits`
before the agent edits code.

For a shareable agreement artifact, save the raw `agent-route` JSON and run:

```bash
scripts/codebase-memory-bridge-report.sh \
  --backend-evidence /tmp/codeinsight-backend-evidence.json \
  --agent-route-json /tmp/codeinsight-agent-route.json \
  --output-dir /tmp/codeinsight-codebase-memory-bridge
```

Aggregate multiple bridge reports into one cohort summary:

```bash
scripts/codebase-memory-bridge-cohort-summary.sh \
  /tmp/codeinsight-codebase-memory-bridge/task-1 \
  /tmp/codeinsight-codebase-memory-bridge/task-2 \
  --min-reports 2 \
  --min-backend-agreement-rate 100 \
  --check
```

Or generate all per-task reports and the aggregate summary from a TSV manifest:

```bash
scripts/codebase-memory-bridge-cohort-report.sh \
  --manifest /tmp/codeinsight-codebase-memory-bridge.tsv \
  --output-dir /tmp/codeinsight-codebase-memory-bridge-cohort \
  --min-reports 3 \
  --min-backend-agreement-rate 100 \
  --check
```

Manifest rows use:

```text
slug<TAB>task<TAB>backend_evidence_json<TAB>agent_route_json
```

See the checked-in
[codebase-memory bridge cohort example](docs/codebase-memory-bridge-cohort-example.md)
for a 3-task maintainer run where codebase-memory supplied candidate files and
CodeInsight matched the backend top file in all reports while preserving
advisory route-quality checks.

The cohort JSON also reports `backend_route_agreement_rate` and
`backend_route_agreement_counts` for `agree`, `overlap`, `conflict`,
`backend_only`, `no_local_route`, and `backend_without_candidates`. With
`--check`, `--min-backend-agreement-rate` gates whether enough reports have
machine-readable `backend_route_agreement.status == "agree"`.

The same object can be passed as MCP `backend_evidence`. See
[Backend adapter strategy](docs/backend-adapter-strategy.md) for the evidence
shape and conflict-handling rule.

For a large repository or a known subsystem, scope the local index before
collecting evidence:

```toml
# /path/to/repo/.codeinsight/config.toml
[index]
include = ["packages/api/**", "src/**"]
exclude = ["**/*.generated.ts", "fixtures/**"]
```

Then confirm the scope and run evidence collection with explicit seeds:

```bash
codeinsight config-status /path/to/repo
scripts/local-repo-evidence.sh /path/to/repo \
  --file packages/api/src/main.ts \
  --symbol main \
  --output /tmp/codeinsight-local-evidence.md \
  --json /tmp/codeinsight-agent-route.json \
  --summary-json /tmp/codeinsight-local-evidence.json
```

The evidence Markdown and summary JSON include `index_scope_enabled`,
`index_scope_includes`, `index_scope_excludes`, and `index_scope_roots`, so
reviewers can see the exact indexing scope behind the route-quality numbers.

Generate a blind-read vs routed-first-read comparison for adoption notes:

```bash
scripts/adoption-comparison.sh /path/to/repo \
  --expected-file src/router.ts \
  --output-dir /tmp/codeinsight-adoption-comparison
```

It writes `adoption-comparison.md`, `summary.json`, the local evidence summary,
and the raw `agent_route` JSON. Use it when you need to show how many source
lines CodeInsight avoided before the agent starts opening files. Add repeated
`--expected-file` values for task-critical files; the report then records
coverage in `task_coverage` and fails when routed first-read context misses one.

The demo executes the same product path an MCP client should follow:
`agent_route`, which runs `index_project -> project_overview -> context_pack ->
impact_analysis` in one first-read route. It prints index timing, entrypoint and
recommendation counts, selected context size, reading-plan steps,
reading-plan questions, reading-plan reasons, candidate selection rank,
selection evidence, line-reduction percentage, execution-plan actions, the first
executable suggested tool, continuation status, continuation next action,
omitted-candidate follow-up status, impact-analysis summary, and a short talk
track for recordings or project introductions. The important demo signal is not
just which file was selected, but what question it should answer, why the agent
should read it first, when a local tool is safe to offer, whether more context
is available after the selected pack, and what impact check should happen before
edits.

For a recording or project introduction, use the
[public demo one-pager](docs/public-demo-one-pager.md),
[two-minute demo script](docs/demo-script.md), and the checked-in
[demo output snapshot](docs/demo-output.md).

## Current Evidence

The checked-in benchmark reports exercise real public repositories and measure
how much source text `context_pack` keeps under a 6000-token budget, plus the
entrypoint and recommended-tool routing decisions from `project_overview`:

- [Smoke benchmark](docs/benchmark-v0.1.md): p-limit, itsdangerous, Go example,
  and memchr.
- [Large repository benchmark](docs/benchmark-large.md): express, Flask, Gin,
  and Tokio.
- [Adoption cases](docs/adoption-cases.md): public repository blind-read vs
  routed-first-read comparison summary across JavaScript, Go, Rust, Python, and
  multi-language library repositories, including a heavyweight Django routing
  case.
- [CodeInsight self adoption report](docs/adoption-report-codeinsight.md): a
  complete report bundle snapshot with issue template, manifest, raw MCP
  first-call JSON, and diagnostic logs.
- [Benchmark methodology](docs/benchmark-methodology.md): what the reports
  prove, refresh commands, profile knobs, and guardrails.

These are first-read routing reports, not controlled performance claims. They
verify that CodeInsight can index real repositories and return bounded context
packs without external services. Treat the reduction numbers as evidence of
agent workflow focus and token discipline, not as runtime performance, parser
accuracy, or proof that unselected code is irrelevant.

Current benchmark snapshot:

- The two-minute demo for this repository shows the agent route selecting 553 of 93,880 source lines, avoiding 93,327 source lines before broad reading for a 99.4% reduction and 169.8x read-less ratio, then surfacing candidate rank 1, reporting high route quality from 24 evidence signals, mirroring `current_reading_step` to `reading_plan[0]`, carrying read-less instruction evidence in `execution_plan[0]`, gating `file_outline` behind the selected-context read, and reporting continuation status before the impact check.
- Smoke repositories route `context_pack` first for 4/4 repositories and
  select 709 of 75,753 source lines, a 99.1% aggregate line reduction.
- Large repositories route `context_pack` first for 4/4 repositories and
  select 1,781 of 241,724 source lines, a 99.3% aggregate line reduction.
- The adoption case summary covers 7 public repositories and routes a first read
  to 2,916 of 682,538 source lines, avoiding 679,622 lines before broad file
  reading, a 99.6% aggregate reduction and 234.1x aggregate read-less ratio.
- The CodeInsight self adoption report packages a full tar.gz handoff and
  routes the entrypoint task to 540 of 78,072 source lines, avoiding 77,532
  source lines before broad reading for a 99.3% reduction and 144.6x read-less
  ratio, with 7 type-relation edges surfaced through the `base_type` graph
  filter, reporting high route quality from 7 evidence signals, while the MCP
  first-call contract fields all pass, including the `current_reading_step` mirror and read-less instruction evidence.
- The public route-quality snapshot pins Express, FastAPI, Flask, Gin,
  Requests, Streamlit, and Wouter and passes 92/92 expected first-file checks,
  selecting 41,664 of 7,142,226 task source lines, a 99.41% aggregate
  first-read line reduction, and records the first suggested tool for each
  route.
- An optional heavyweight Django route-quality case is available with
  `scripts/public-task-routing-matrix.sh --case django`; the pinned manual probe
  covers URL resolver routing, request/response lifecycle, and middleware
  execution, passes 3/3 expected first-file checks, and shows a 99.87%
  aggregate first-read line reduction in the latest local verification.
- Per-repository adoption metrics, commits, and refresh commands live in
  [Adoption cases](docs/adoption-cases.md).
- `adoption-comparison.sh --expected-file` turns a real task's known critical
  files into a first-read coverage gate, so adoption evidence can show whether
  the route selected the files an agent should inspect before editing. The
  checked-in [task coverage evidence](docs/task-coverage-evidence.md) shows a
  maintainer task selecting `541` of `82,045` source lines with `1/1` critical
  file coverage.
- Generated reports include a `Key Results` section with routing,
  compression, token-budget, indexing, guardrail, and truncation evidence.
- Per-repository details include a `Context reading plan` table with candidate
  rank, the local reading question, first-read reason, and raw selection reason,
  so the benchmark shows why the context router chose each file instead of only
  reporting compression numbers.
- The MCP stdio smoke executes `agent_route.execution_plan[].suggested_tool`
  through `tools/call`, proving the follow-up is usable by clients instead of
  only display metadata.
- The impact execution step mirrors `impact_analysis.suggested_checks` and
  exposes an `impact_analysis` suggested tool so clients can render a pre-edit
  checklist without digging through nested fields.
- The MCP first-call smoke also verifies the empty-repository path returns
  `blocked_no_seed` with `provide_seed_file_or_symbol`, so clients can ask for a
  seed instead of falling back to blind reads.

Adoption evidence snippet for your own repository:

```bash
scripts/adoption-evidence.sh /path/to/repo \
  --output-dir /tmp/codeinsight-adoption-evidence \
  --print-snippet \
  --issue-template
```

Successful output produces `/tmp/codeinsight-adoption-evidence/adoption-evidence.md`
and `/tmp/codeinsight-adoption-evidence/issue-template.md`, then prints this copyable shape:

```text
# CodeInsight Adoption Evidence

- Status: `pass`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Selected context: `<selected>/<total>` source lines, `<reduction>` reduction
- Seed strategy: `<seed_strategy>`
- Selected seeds: `<count>`
- First seed source: `<source>`
- Companion entrypoint: `<file-or-dash>`
- First selected file: `<file>`
- First reading focus: <focus>
- First reading question: <question>
- First selection rank: `<rank>`
- First selection reason: <reason>
- Continuation status: `<status>`
- Continuation next action: `<next_action>`
- First omitted candidate: `<file-or-none>`
- MCP server: `codeinsight`
- MCP route quality: `<level>` (`<score>/100`, `<evidence>` evidence signals), next=`<action>`
- MCP first-call contract: reading_order=`true`, current_reading_step=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`
- First-read gating: suggested_tool_after_selected_context=`true`, continuation_after_selected_context=`true`, impact_review_before_edits=`true`
- MCP suggested tool executed: `true`
- MCP impact status: `complete`
```

Use `issue-template.md` when filing a reproducible adoption report with the
summary snippet, failure category placeholder, artifact paths, and environment
fields already filled in. Use `summary.json` from the same folder when CI,
README automation, or issue templates need the same result without parsing
Markdown; it includes `first_read_gating` for selected-context, continuation,
and impact-review ordering.

For non-maintainer feedback, use the external Beta wrapper:

```bash
scripts/external-beta-trial.sh /path/to/repo \
  --task "understand the main application entrypoint" \
  --output-dir /tmp/codeinsight-external-beta-trial
```

It writes `issue-body.md`, `beta-summary.json`, `redaction-checklist.md`, and
`maintainer-triage.md` beside the underlying adoption evidence. External users
can choose `needs_triage` when they are unsure whether the result is a
`route_hit`, `route_miss`, or workflow issue.

Aggregate three External Beta reports before a public handoff:

```bash
scripts/external-beta-cohort-summary.sh \
  /tmp/codeinsight-external-beta-trial-1 \
  /tmp/codeinsight-external-beta-trial-2 \
  /tmp/codeinsight-external-beta-trial-3 \
  --output /tmp/codeinsight-external-beta-cohort.md \
  --json /tmp/codeinsight-external-beta-cohort.json \
  --min-route-quality-score 70 \
  --check
```

The cohort summary reports `status`, outcome counts, `needs_triage` count, and
the next action priority: triage unresolved reports, fix workflow friction, fix
low route quality, fix route misses, address over-trust wording, or publish the
Beta summary.

Turn the cohort into a maintainer work queue:

```bash
scripts/external-beta-fix-queue.sh \
  /tmp/codeinsight-external-beta-cohort.json \
  --output /tmp/codeinsight-external-beta-fix-queue.md \
  --json /tmp/codeinsight-external-beta-fix-queue.json \
  --check
```

Or package the cohort summary and fix queue into one handoff folder:

```bash
scripts/external-beta-cohort-report.sh \
  /tmp/codeinsight-external-beta-trial-1 \
  /tmp/codeinsight-external-beta-trial-2 \
  /tmp/codeinsight-external-beta-trial-3 \
  --output-dir /tmp/codeinsight-external-beta-handoff \
  --min-route-quality-score 70 \
  --check \
  --print-snippet
```

The handoff folder contains `external-beta-cohort.md`,
`external-beta-cohort-summary.json`, `external-beta-fix-queue.md`,
`external-beta-fix-queue.json`, `external-beta-handoff-issue.md`,
`manifest.json`, and a short `README.md` with the cohort status and next action.
`external-beta-handoff-issue.md` is the ready-to-file GitHub issue or discussion
body; `--print-snippet` prints the same public handoff status,
route-quality gate, next action, fix queue, and artifact paths.

Use `scripts/adoption-report.sh` when you need one uploadable archive. It writes
`codeinsight-adoption-report.tar.gz` with `adoption-evidence.md`,
`issue-template.md`, `summary.json`, raw route JSON, MCP first-call JSON, a
manifest, and diagnostic stdout/stderr logs.

Refresh the reports locally:

```bash
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

For subset runs and guardrail details, see
[Benchmark methodology](docs/benchmark-methodology.md).

Run the same benchmark shape against your own repository:

```bash
CODEINSIGHT_BENCH_PROFILE=local \
  CODEINSIGHT_BENCH_LOCAL_ROOT=/path/to/repo \
  CODEINSIGHT_BENCH_LOCAL_CONTEXT_FILE=src/main.ts \
  CODEINSIGHT_BENCH_LOCAL_TASK="understand the app entrypoint" \
  CODEINSIGHT_BENCH_OUTPUT=/tmp/codeinsight-local-benchmark.md \
  scripts/benchmark-smoke.sh
```

Key docs:

- [Quickstart](docs/quickstart.md)
- [Adoption checklist](docs/adoption-checklist.md)
- [Demo script](docs/demo-script.md)
- [Demo output snapshot](docs/demo-output.md)
- [Adoption cases](docs/adoption-cases.md)
- [Task routing matrix](docs/task-routing-matrix.md)
- [Public task routing matrix](docs/public-task-routing-matrix.md)
- [Public task routing matrix JSON](docs/public-task-routing-matrix-summary.json)
- [codebase-memory-mcp comparison](docs/competitive-analysis-codebase-memory.md)
- [Backend adapter strategy](docs/backend-adapter-strategy.md)
- [CodeInsight self adoption report](docs/adoption-report-codeinsight.md)
- [Django adoption case](docs/adoption-case-django.md)
- [Express adoption case](docs/adoption-case-express.md)
- [Gin adoption case](docs/adoption-case-gin.md)
- [ip2region adoption case](docs/adoption-case-ip2region.md)
- [Memchr adoption case](docs/adoption-case-memchr.md)
- [Requests adoption case](docs/adoption-case-requests.md)
- [Agent prompt templates](docs/agent-prompt-template.md)
- [Client integration examples](docs/client-integration-examples.md)
- [Current status](docs/status.md)
- [Maintainer checklist](docs/maintainer-checklist.md)
- [Maintenance commands](docs/maintenance-commands.md)
- [Install](docs/install.md)
- [CLI usage](docs/cli-usage.md)
- [MCP tools](docs/mcp-tools.md)
- [First-read workflow](docs/first-read-workflow.md)
- [Client workflow](docs/client-workflow.md)
- [Release commands](docs/release-commands.md)
- [Release readiness](docs/release-readiness.md)
- [Known limitations](docs/known-limitations.md)
- [Documentation index](docs/README.md)
- [MVP public readiness](docs/mvp-public-readiness.md)
- [Public Adoption Alpha](docs/public-adoption-alpha.md)
- [External Beta trial](docs/external-beta-trial.md)
- [External Beta handoff example](docs/external-beta-handoff-example.md)
- [Alpha feedback triage](docs/alpha-feedback-triage.md)
- [Alpha trial log](docs/alpha-trial-log.md)
- [Public adoption feedback template](docs/public-adoption-feedback-template.md)

## Current Status

CodeInsight is an early MVP for local-first AI-agent repository reading. It can
route a broad task through `agent_first_read`, select bounded context, return a
reading plan, hand off an executable suggested tool, and require a focused
impact check before edits.

Latest verified release: `v0.1.12`.

Current public route-quality evidence passes `92/92` first-file checks and
selects 41,664 of 7,142,226 task source lines.

Next focus: use the external Beta trial wrapper, cohort summary, and fix queue
to collect at least three non-maintainer reports, then fix the highest priority
workflow, route-quality, route-miss, or wording item.
See [Current status](docs/status.md) for the full implemented capability list.

## Install From Release

Install the latest macOS or Linux release:

```bash
curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | sh
```

For version pinning, custom install directories, authenticated downloads, and
installer smoke tests, see [Install](docs/install.md).

For the full install-to-agent path, see [Quickstart](docs/quickstart.md).

## Verify Adoption

After installing `codeinsight`, verify the user-side first-read route:

```bash
CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh
```

This smoke proves the installed binary can run `version`, `index`, `overview`,
`context-pack`, CLI `agent-route`, MCP stdio, and MCP `agent_route` against a
temporary project outside the source checkout. It also verifies the returned
agent-route execution plan, reading-plan question, reading-plan reason,
reading-plan focus, selection rank, selection reason, and continuation evidence
that agents use to read files in the right order.

For the complete adoption checklist, see
[Adoption checklist](docs/adoption-checklist.md).

## Install With Homebrew

```bash
brew tap sleticalboy/tap
brew install codeinsight
```

## Install From Source

```bash
cargo install --path .
```

## Run With Docker

```bash
docker pull ghcr.io/sleticalboy/codeinsight-mcp:latest
docker run --rm -v "$PWD:/workspace" ghcr.io/sleticalboy/codeinsight-mcp:latest overview /workspace
```

For local image builds, platform details, and Docker smoke tests, see
[Install](docs/install.md).

## CLI Usage

Run the default first-read route, or inspect each routing step manually:

```bash
codeinsight agent-route /path/to/repo --task "understand app entrypoint" --token-budget 6000
cargo run -- index /path/to/repo --force
cargo run -- overview /path/to/repo
cargo run -- context-pack /path/to/repo --task "understand app entrypoint" --token-budget 6000
```

Start the MCP stdio server:

```bash
cargo run -- serve --transport stdio
```

For all commands and common workflows, see [CLI usage](docs/cli-usage.md).

## MCP Tools

Recommended MCP first-read flow:

1. Call `agent_first_read` with `root`, `task`, and `token_budget` for the
   default first-read path. If the task text names an indexed file path such as
   `src/auth.ts`, CodeInsight treats it as an automatic file seed. The response
   is compact, defers impact analysis, and uses an 8000-token structured
   response budget by default.
2. If `context_pack.continuation_summary.status` is `blocked_no_seed`, ask for
   a seed file or symbol and retry `agent_first_read` instead of broad-reading the
   repository. If it is `blocked_invalid_seed`, ask for an existing seed file
   under the project root or a symbol, then retry. If it is
   `blocked_no_context`, ask for a seed file or symbol that actually matches
   indexed source context. If it is `blocked_unindexed_task_path`, update the
   index scope or rerun indexing so the task path is indexed before retrying.
3. Use `agent_first_read.current_reading_step` as the first checklist row, then read
   `context_pack.files[]` in `reading_plan[]` order. Use
   `agent_first_read.routing_decision` when the client needs one compact object for
   the first seed, first file, read-less metrics, continuation state, and
   impact status.
4. Use `reading_plan[].selection_rank` and `selection_reason` as the audit
   trail, and use `continuation_summary` only after selected context has been
   consumed.
5. Show `context_pack.read_less` when the client needs a direct source-line
   baseline, selected-line count, avoided-line count, reduction percentage, and
   read-less ratio.
6. Use advanced `agent_route`, or the lower-level `index_project`,
   `project_overview`, `context_pack`, and `impact_analysis` tools, when the
   client needs custom routing, backend evidence, or partial refresh control.

For the full tool list, `tools/call` examples, topic contracts, and accuracy
boundaries, see [MCP tools](docs/mcp-tools.md). For client setup snippets, see
[MCP client configuration](docs/mcp-client-config.md).

## Development

```bash
scripts/local-ci-smoke.sh
```

For focused smoke groups, benchmark checks, and optional external checks, see
[Maintenance commands](docs/maintenance-commands.md).

## Release Builds

The `Release Build` workflow supports manual artifact builds and tagged GitHub
releases. See [Release runbook](docs/release-runbook.md).

Maintainer release path:

```bash
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp --evidence-file release-evidence/vX.Y.Z.md --evidence-json-file release-evidence/vX.Y.Z.json vX.Y.Z main
scripts/prepare-release.sh --dry-run vX.Y.Z
scripts/prepare-release.sh vX.Y.Z
git push origin main
scripts/release-pretag-check.sh main
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX.Y.Z.json vX.Y.Z main
scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
scripts/post-release-verify.sh --handoff vX.Y.Z
```

## License

CodeInsight MCP Server is licensed under the Apache License 2.0.

The first MVP intentionally avoids external services such as Qdrant, pgvector, Neo4j, or Apache AGE. The default path must remain local, single-binary, and low configuration.
