# CodeInsight MCP Server

CodeInsight MCP Server is a local-first first-read router for AI coding agents.
It turns "scan the repository and guess what matters" into a bounded local route:
index the project, choose the first context pack, offer the next focused tool,
and preview impact before edits.

The product is intentionally narrow. It is built for the first-read problem:
before an agent edits a repository, it needs to know which files matter, what
entrypoints to inspect, what context fits the token budget, and what may be
impacted by a change. CodeInsight keeps that loop local and exposes it through
CLI and MCP tools.

The primary agent loop is one call:

```text
agent_route -> selected context -> executable suggested_tool -> impact check
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
- an `impact_analysis` preview before edits

CodeInsight is not trying to replace an IDE, LSP, compiler, or Sourcegraph. Its
job is to help AI agents read less code, pick better context, and stop guessing
which file to open next.

Use CodeInsight when the agent needs a local reading route. Keep using the IDE,
LSP, compiler, test runner, and language-specific tools for precise diagnostics,
type checks, and final verification.

Current public routing snapshot: pinned Express, Flask, Gin, and Requests
route-quality expectations pass `15/15`, selecting 7,236 of 279,685 task source
lines for a `97.4%` aggregate first-read line reduction. See the checked-in
[Markdown snapshot](docs/public-task-routing-matrix.md) and
[JSON summary](docs/public-task-routing-matrix-summary.json). Reproduce both:

```bash
scripts/update-public-task-routing-matrix.sh --check
```

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
   Call agent_route with root, task, and token_budget 6000 before reading files directly.
   Read selected files in reading_plan order and use selection_rank as the audit trail.
   Use reading_plan.focus as the compact scan label for the selected file.
   Treat reading_plan.question as the local checklist for the selected file.
   Use context_pack.read_less only as first-read reduction evidence.
   Use continuation_summary only after selected context is consumed.
   Follow agent_route.execution_plan[] in order.
   ```

   The first MCP `tools/call` payload is shown in
   [First Agent Route Call](docs/mcp-client-config.md#first-agent-route-call).

4. Pick the validation that matches your adoption stage:

   | Stage | Command | Use When |
   | --- | --- | --- |
   | First look | `scripts/two-minute-demo.sh` | You want a visible `agent_route -> context_pack -> impact_analysis` walkthrough with an `[Evidence summary]`. |
   | Framework entrypoints | `scripts/framework-entrypoint-demo.sh` | You want local proof that Next.js, Rails, Django, and C# web entrypoints can be detected and routed as first context for matching tasks. |
   | Task routing matrix | `scripts/task-routing-matrix.sh /path/to/repo --expect-file ./route-expectations.tsv` | You want a multi-task first-read matrix showing first selected file, seed strategy, line reduction, token estimate, impact preview, and optional expected-file gates for one repository. |
   | Public route matrix | `scripts/public-task-routing-matrix.sh` | You want one aggregate route-quality summary across pinned checked-in Express, Flask, Gin, and Requests expectation files. See the checked-in [public routing snapshot](docs/public-task-routing-matrix.md); add `--case streamlit` when you want the larger Streamlit case. |
   | MCP wiring | `CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/mcp-first-call-smoke.sh` | You want a compact JSON proof that stdio MCP accepts `agent_route`, returns the first context file, follows `reading_plan[]`, exposes read-less metrics, selection rank, and continuation evidence, runs the current step's suggested tool, and includes `impact_status`. |
   | Installed adoption | `CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh` | You want the installed binary to pass CLI `agent-route`, MCP stdio, and MCP `agent_route` against a temporary project with read-less, selection-rank, and continuation evidence. |
   | Local evidence | `scripts/adoption-evidence.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-evidence --print-snippet --issue-template` | You want one folder with local first-read evidence, raw route JSON, MCP first-call JSON, aggregate Markdown/JSON summaries, a copyable terminal snippet, and a ready-to-file issue template. |
   | Adoption comparison | `scripts/adoption-comparison.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-comparison` | You want a shareable blind-read vs routed-first-read comparison showing source lines avoided, read-less ratio, seed strategy, first reading focus/question, selection rank, and continuation next action. |
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

Generate a blind-read vs routed-first-read comparison for adoption notes:

```bash
scripts/adoption-comparison.sh /path/to/repo \
  --output-dir /tmp/codeinsight-adoption-comparison
```

It writes `adoption-comparison.md`, `summary.json`, the local evidence summary,
and the raw `agent_route` JSON. Use it when you need to show how many source
lines CodeInsight avoided before the agent starts opening files.

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
[two-minute demo script](docs/demo-script.md) and the checked-in
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
  routed-first-read comparison summary across JavaScript, Go, Rust, and Python
  public repositories.
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

- The two-minute demo for this repository shows the agent route selecting 438 of 33,945 source lines, avoiding 33,507 source lines before broad reading for a 98.7% reduction and 77.5x read-less ratio, then surfacing candidate rank 1, mirroring `current_reading_step` to `reading_plan[0]`, carrying read-less instruction evidence in `execution_plan[0]`, gating `file_outline` behind the selected-context read, and reporting continuation status before the impact check.
- Smoke repositories route `context_pack` first for 4/4 repositories and
  select 629 of 75,753 source lines, a 99.2% aggregate line reduction.
- Large repositories route `context_pack` first for 4/4 repositories and
  select 1,748 of 241,555 source lines, a 99.3% aggregate line reduction.
- The adoption case summary covers 4 public repositories and routes a first read
  to 1,361 of 126,990 source lines, avoiding 125,629 lines before broad file
  reading, a 98.9% aggregate reduction and 93.3x aggregate read-less ratio.
- The CodeInsight self adoption report packages a full tar.gz handoff and
  routes the entrypoint task to 440 of 33,945 source lines, avoiding 33,505
  source lines before broad reading for a 98.7% reduction and 77.1x read-less
  ratio, with 7 type-relation edges surfaced through the `base_type` graph
  filter, while the MCP first-call contract fields all pass, including the
  `current_reading_step` mirror and read-less instruction evidence.
- The public route-quality snapshot pins Express, Flask, Gin, and Requests and
  passes 15/15 expected first-file checks, selecting 7,236 of 279,685 task
  source lines, a 97.4% aggregate first-read line reduction.
- Per-repository adoption metrics, commits, and refresh commands live in
  [Adoption cases](docs/adoption-cases.md).
- Generated reports include a `Key Results` section with routing,
  compression, token-budget, indexing, guardrail, and truncation evidence.
- Per-repository details include a `Context reading plan` table with candidate
  rank, the local reading question, first-read reason, and raw selection reason,
  so the benchmark shows why the context router chose each file instead of only
  reporting compression numbers.
- The MCP stdio smoke executes `agent_route.execution_plan[].suggested_tool`
  through `tools/call`, proving the follow-up is usable by clients instead of
  only display metadata.

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
- [CodeInsight self adoption report](docs/adoption-report-codeinsight.md)
- [Express adoption case](docs/adoption-case-express.md)
- [Gin adoption case](docs/adoption-case-gin.md)
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

## Current Status

CodeInsight is an early MVP. It can index local repositories, expose CLI and
MCP navigation tools, build agent-ready context packs, run local impact
analysis, and optionally use configured semantic embeddings.

Latest verified release: `v0.1.12`.

Next focus: strengthen real-repository demos and README evidence around the
AI-agent first-read workflow.
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

1. Call `agent_route` with `root`, `task`, and `token_budget` for the default
   first-read path.
2. Use `agent_route.current_reading_step` as the first checklist row, then read
   `context_pack.files[]` in `reading_plan[]` order.
3. Use `reading_plan[].selection_rank` and `selection_reason` as the audit
   trail, and use `continuation_summary` only after selected context has been
   consumed.
4. Show `context_pack.read_less` when the client needs a direct source-line
   baseline, selected-line count, avoided-line count, reduction percentage, and
   read-less ratio.
5. Use `index_project`, `project_overview`, `context_pack`, and
   `impact_analysis` directly when the client needs custom routing or partial
   refresh control.

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
