# Documentation

This directory contains the detailed product, user, tool, validation, and
release documents for CodeInsight.

## Start Here

Default adoption path:

1. Follow [Quickstart](quickstart.md).
2. Configure a local stdio MCP server with
   [MCP client configuration](mcp-client-config.md).
3. Use `agent_route` as the default first-read route.
4. Verify the installed binary with `scripts/installed-quickstart-smoke.sh`.
5. Finish with the [Adoption checklist](adoption-checklist.md).

- [Quickstart](quickstart.md)
- [Adoption checklist](adoption-checklist.md)
- [MVP public readiness](mvp-public-readiness.md)
- [Public Adoption Alpha](public-adoption-alpha.md)
- [External Beta trial](external-beta-trial.md)
- [Alpha feedback triage](alpha-feedback-triage.md)
- [Alpha trial log](alpha-trial-log.md)
- [Public adoption feedback template](public-adoption-feedback-template.md)
- [Public demo one-pager](public-demo-one-pager.md)
- [Demo script](demo-script.md)
- [Demo output snapshot](demo-output.md)
- [Adoption cases](adoption-cases.md)
- [Task routing matrix](task-routing-matrix.md)
- [Public task routing matrix](public-task-routing-matrix.md) and
  [JSON summary](public-task-routing-matrix-summary.json)
- [codebase-memory bridge cohort example](codebase-memory-bridge-cohort-example.md)
- Task routing expectations:
  [Django](task-routing-expectations/django.tsv),
  [Express](task-routing-expectations/express.tsv),
  [FastAPI](task-routing-expectations/fastapi.tsv),
  [Flask](task-routing-expectations/flask.tsv),
  [Gin](task-routing-expectations/gin.tsv),
  [Requests](task-routing-expectations/requests.tsv),
  [Streamlit](task-routing-expectations/streamlit.tsv),
  [Wouter](task-routing-expectations/wouter.tsv)
- [CodeInsight self adoption report](adoption-report-codeinsight.md)
- [Django adoption case](adoption-case-django.md)
- [Express adoption case](adoption-case-express.md)
- [Gin adoption case](adoption-case-gin.md)
- [ip2region adoption case](adoption-case-ip2region.md)
- [Memchr adoption case](adoption-case-memchr.md)
- [Requests adoption case](adoption-case-requests.md)
- [Agent prompt templates](agent-prompt-template.md)
- [Client integration examples](client-integration-examples.md)
- [Current status](status.md)
- [Maintainer checklist](maintainer-checklist.md)
- [Maintenance commands](maintenance-commands.md)
- [Install](install.md)
- [First-read workflow](first-read-workflow.md)
- [CLI usage](cli-usage.md)
- [MCP tools](mcp-tools.md)
- [MCP client configuration](mcp-client-config.md) for Codex, Claude Code,
  Cursor, and generic MCP JSON clients
- [Client workflow](client-workflow.md)
- [Known limitations](known-limitations.md)

## Product And Planning

- [Product prototype](product-prototype.md)
- [Implementation plan](implementation-plan.md)
- [MVP backlog](mvp-backlog.md)

## Tool Contracts

- [First-read workflow](first-read-workflow.md)
- [Client workflow](client-workflow.md)
- [Agent prompt templates](agent-prompt-template.md)
- [Client integration examples](client-integration-examples.md)
- [Recommendation contract](recommendation-contract.md)
- [Navigation tools](navigation-tools.md)
- [Impact analysis](impact-analysis.md)
- [Embedding providers](embedding-providers.md)

## Validation

Choose the check by adoption stage:

| Stage | Command |
| --- | --- |
| Product walkthrough | `scripts/two-minute-demo.sh` |
| Copyable MCP first-call JSON | `scripts/mcp-first-call-smoke.sh` |
| MCP client wiring | `scripts/mcp-stdio-smoke.sh` |
| Installed-binary adoption gate | `scripts/installed-quickstart-smoke.sh` |
| External Beta trial pack | `scripts/external-beta-trial.sh /path/to/repo --output-dir /tmp/codeinsight-external-beta-trial` |
| External Beta cohort summary | `scripts/external-beta-cohort-summary.sh /tmp/beta-1 /tmp/beta-2 /tmp/beta-3 --check` |
| Adoption comparison evidence | `scripts/adoption-comparison.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-comparison` |
| Uploadable adoption report | `scripts/adoption-report.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-report` |
| Local task routing matrix | `scripts/task-routing-matrix.sh /path/to/repo --expect-file ./route-expectations.tsv` |
| Public route-quality snapshot | `scripts/update-public-task-routing-matrix.sh` |
| codebase-memory bridge cohort | `scripts/codebase-memory-bridge-cohort-summary.sh /tmp/task-1 /tmp/task-2 /tmp/task-3 --min-reports 3 --check` |
| Local repository benchmark | `CODEINSIGHT_BENCH_PROFILE=local ... scripts/benchmark-smoke.sh` |

- [MVP public readiness](mvp-public-readiness.md)
- [Public Adoption Alpha](public-adoption-alpha.md)
- [External Beta trial](external-beta-trial.md)
- [Alpha feedback triage](alpha-feedback-triage.md)
- [Alpha trial log](alpha-trial-log.md)
- [Public adoption feedback template](public-adoption-feedback-template.md)
- [Public demo one-pager](public-demo-one-pager.md)
- [Maintenance commands](maintenance-commands.md)
- Two-minute demo: `scripts/two-minute-demo.sh`
- MCP first-call JSON summary: `scripts/mcp-first-call-smoke.sh`
- Installed first-read route: `scripts/installed-quickstart-smoke.sh`
- Adoption comparison evidence: `scripts/adoption-comparison.sh`
- Agent-router lower-level metrics, rank evidence, and continuation action:
  `scripts/agent-router-demo.sh`
- [Two-minute demo script](demo-script.md)
- [Two-minute demo output snapshot](demo-output.md)
- [MCP client smoke test](mcp-client-smoke.md)
- [Semantic smoke test](semantic-smoke.md)
- [Benchmark methodology](benchmark-methodology.md)
- [Adoption cases](adoption-cases.md)
- [CodeInsight self adoption report](adoption-report-codeinsight.md)
- [Django adoption case](adoption-case-django.md)
- [Express adoption case](adoption-case-express.md)
- [Gin adoption case](adoption-case-gin.md)
- [ip2region adoption case](adoption-case-ip2region.md)
- [Memchr adoption case](adoption-case-memchr.md)
- [Requests adoption case](adoption-case-requests.md)
- [Smoke benchmark](benchmark-v0.1.md)
- [Large repository benchmark](benchmark-large.md)
- [codebase-memory bridge cohort example](codebase-memory-bridge-cohort-example.md)

## Release

- [Maintainer checklist](maintainer-checklist.md)
- [Release commands](release-commands.md)
- [Release readiness](release-readiness.md)
- [Release runbook](release-runbook.md)
- [Changelog](../CHANGELOG.md)
