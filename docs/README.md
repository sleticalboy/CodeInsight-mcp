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
- [Demo script](demo-script.md)
- [Demo output snapshot](demo-output.md)
- [Adoption cases](adoption-cases.md)
- [Express adoption case](adoption-case-express.md)
- [Gin adoption case](adoption-case-gin.md)
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
| Adoption comparison evidence | `scripts/adoption-comparison.sh /path/to/repo --output-dir /tmp/codeinsight-adoption-comparison` |
| Local repository benchmark | `CODEINSIGHT_BENCH_PROFILE=local ... scripts/benchmark-smoke.sh` |

- [Maintenance commands](maintenance-commands.md)
- Two-minute demo: `scripts/two-minute-demo.sh`
- MCP first-call JSON summary: `scripts/mcp-first-call-smoke.sh`
- Installed first-read route: `scripts/installed-quickstart-smoke.sh`
- Adoption comparison evidence: `scripts/adoption-comparison.sh`
- Agent-router lower-level metrics and reasons: `scripts/agent-router-demo.sh`
- [Two-minute demo script](demo-script.md)
- [Two-minute demo output snapshot](demo-output.md)
- [MCP client smoke test](mcp-client-smoke.md)
- [Semantic smoke test](semantic-smoke.md)
- [Benchmark methodology](benchmark-methodology.md)
- [Adoption cases](adoption-cases.md)
- [Express adoption case](adoption-case-express.md)
- [Gin adoption case](adoption-case-gin.md)
- [Requests adoption case](adoption-case-requests.md)
- [Smoke benchmark](benchmark-v0.1.md)
- [Large repository benchmark](benchmark-large.md)

## Release

- [Maintainer checklist](maintainer-checklist.md)
- [Release commands](release-commands.md)
- [Release readiness](release-readiness.md)
- [Release runbook](release-runbook.md)
- [Changelog](../CHANGELOG.md)
