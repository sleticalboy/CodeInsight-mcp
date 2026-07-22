# Public Adoption Alpha

Public Adoption Alpha is the next milestone after the public MVP gate. The goal
is to make CodeInsight usable by external AI-coding-agent users, collect
reproducible feedback, and keep the product story centered on local-first
first-read context routing.

## Positioning

CodeInsight is a local-first MCP code context router for AI coding agents. It
helps an agent decide what to read first, why that context was selected, which
tool to call next, and what impact to inspect before edits.

It is not an IDE, LSP, compiler, hosted index, or enterprise collaboration
platform.

## Alpha Success Criteria

- A new user can install `codeinsight`, configure the local stdio MCP server,
  run a demo, and call `agent_route` on a repository in about 10 minutes.
- The user can attach one feedback report containing the task, first selected
  file, route metrics, and whether the first read was useful.
- Maintainers can reproduce feedback with raw `agent_route` JSON or an
  adoption evidence bundle.
- Public repository adoption cases cover more than one language ecosystem and
  include task, expected first-read behavior, actual first selected file, and
  read-less metrics.
- Limitations stay visible before users over-trust static-analysis output.

## 10-Minute Trial Path

1. Install the binary:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | sh
   codeinsight version
   ```

2. Add the MCP server to the client:

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

   Use an absolute `command` path when the client does not inherit shell `PATH`.
   Client-specific examples are in [MCP client configuration](mcp-client-config.md).

3. Run the local product demo from a CodeInsight checkout:

   ```bash
   scripts/two-minute-demo.sh
   ```

4. Run a first-read route on a real repository:

   ```bash
   codeinsight agent-route /path/to/repo \
     --task "understand the main application entrypoint" \
     --token-budget 6000
   ```

5. Generate a shareable evidence folder:

   ```bash
   scripts/adoption-evidence.sh /path/to/repo \
     --output-dir /tmp/codeinsight-adoption-evidence \
     --print-snippet \
     --issue-template
   ```

6. File feedback using [Public adoption feedback template](public-adoption-feedback-template.md).

For non-maintainer Beta feedback, prefer the wrapper in
[External Beta trial](external-beta-trial.md). It generates the same underlying
evidence plus an issue body, redaction checklist, and maintainer triage note.

## Agent Prompt

Use this prompt in Codex, Claude Code, Cursor, or another MCP client:

```text
Use CodeInsight before reading files directly.
Call agent_route with root, task, and token_budget 6000.
Read context_pack.files in reading_plan order.
Use reading_plan.focus as the compact scan label and reading_plan.question as
the local checklist.
Only call the suggested follow-up tool after the selected context file has been
read.
Review impact_analysis before edits.
If continuation_summary.status is blocked_no_seed, ask me for a seed file or
symbol instead of broad-reading the repository.
```

## Evidence To Collect

For every alpha trial, collect:

- repository URL or local project type
- task text
- first selected file
- expected first file or expected area, when known
- selected source lines and blind baseline lines
- read-less ratio and line reduction
- first reading focus and question
- first suggested tool
- impact risk and suggested checks
- whether the first read helped, missed the target, or was misleading

Use `scripts/adoption-evidence.sh --issue-template` when possible; it writes an
issue-ready report with raw route JSON and MCP first-call evidence.

## Current Alpha Evidence

Checked-in adoption cases:

- [Django](adoption-case-django.md): Python web framework URL routing, first
  selected file `django/urls/resolvers.py`, 593 of 529,403 source lines routed
  first, 99.9% reduction, 892.8x read-less ratio.
- [Express](adoption-case-express.md): JavaScript web framework routing.
- [Gin](adoption-case-gin.md): Go web framework routing.
- [ip2region](adoption-case-ip2region.md): multi-language IP lookup library,
  Java search flow, first selected file
  `binding/java/src/main/java/org/lionsoul/ip2region/service/Ip2Region.java`,
  641 of 19,379 source lines routed first, 96.7% reduction, 30.2x read-less
  ratio.
- [Memchr](adoption-case-memchr.md): Rust search implementation flow.
- [Requests](adoption-case-requests.md): Python HTTP session request flow.

The aggregate public adoption snapshot covers six public repositories and
routes 2,595 of 675,772 source lines before broad reading, a 99.6% aggregate
first-read reduction.

Current Alpha Feedback Loop documents:

- [Alpha feedback triage](alpha-feedback-triage.md)
- [Alpha trial log](alpha-trial-log.md)
- [External Beta trial](external-beta-trial.md)
- GitHub `Adoption feedback` issue form under `.github/ISSUE_TEMPLATE/`

Current maintainer-run Alpha trial issues:

- [#1 ip2region Java search flow](https://github.com/sleticalboy/CodeInsight-mcp/issues/1)
- [#2 mcp-hub server routing flow](https://github.com/sleticalboy/CodeInsight-mcp/issues/2)
- [#3 lazy-mcp-wrapper daemon startup route](https://github.com/sleticalboy/CodeInsight-mcp/issues/3)

## Triage Policy

Classify alpha feedback as:

- `route_hit`: first selected file was useful and the reading plan was
  actionable.
- `route_near_miss`: first selected file was in the right area but not the best
  starting point.
- `route_miss`: first selected file was wrong for the task.
- `workflow_friction`: install, MCP config, prompt, or output shape blocked the
  trial.
- `overtrust_risk`: output could be read as compiler-grade or safety proof.

Prioritize fixes in this order:

1. Workflow blockers that prevent a 10-minute trial.
2. Route misses on common frameworks or high-signal public repositories.
3. Output-shape issues that make agents ignore `reading_plan[]` or
   `execution_plan[]`.
4. Documentation gaps that make users over-trust best-effort analysis.

## Maintainer Checks

Before announcing or expanding the alpha:

```bash
scripts/docs-smoke.sh
scripts/local-ci-smoke.sh
scripts/update-adoption-cases.sh --check
gh workflow run CI --ref main
```

The GitHub Actions `CI` run for the target commit must complete successfully.
