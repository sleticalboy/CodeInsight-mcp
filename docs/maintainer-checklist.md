# Maintainer Checklist

Use this checklist for routine CodeInsight maintenance. It links to the deeper
docs when a task needs more detail.

For a short index of local development, maintenance smoke, benchmark, and
optional external checks, see [Maintenance commands](maintenance-commands.md).

## Before Starting Work

- Check the working tree:

  ```bash
  git status --short --branch
  ```

- Review the current product and release state:

  ```bash
  sed -n '1,180p' docs/status.md
  ```

- Keep the product focus narrow: local-first MCP code context routing for AI
  coding agents. For capability boundaries, see
  [Known limitations](known-limitations.md).

## Before Opening A PR

- Run the standard local gate:

  ```bash
  scripts/local-ci-smoke.sh
  ```

- For changes that affect MCP output, semantic behavior, or install behavior,
  also run the relevant smoke from
  [Maintenance commands](maintenance-commands.md):

  ```bash
  scripts/mcp-stdio-smoke.sh
  scripts/semantic-smoke.sh
  scripts/installed-quickstart-smoke.sh
  ```

  `scripts/local-ci-smoke.sh` already includes the context-pack quality smoke
  and agent-router demo for first-read routing and context-packing changes.

- For release-tooling changes, run the release script smokes:

  ```bash
  scripts/release-tooling-smoke.sh
  ```

- For workflow action changes, run the workflow action version smoke:

  ```bash
  scripts/workflow-actions-smoke.sh
  ```

- For README, demo, benchmark, or release-readiness changes, keep the public
  evidence path consistent:

  ```bash
  scripts/docs-smoke.sh
  ```

  Check that the README benchmark snapshot, [Demo script](demo-script.md)
  evidence cutaway, benchmark report `Key Results`, and
  [Release readiness](release-readiness.md) benchmark gate all tell the same
  routing and compression story.

- For benchmark or context-pack quality CI visibility changes, verify the
  generated Actions summary:

  ```bash
  scripts/benchmark-step-summary-smoke.sh
  scripts/context-pack-quality-step-summary-smoke.sh
  ```

  After CI runs, open the `benchmark-subset-smoke` job summary and confirm it
  includes `Key Results`, the `context_pack` summary row, a workflow run link,
  and the `codeinsight-benchmark-subset` artifact link. Download the artifact
  only when you need the full guardrail tables. Open the
  `context-pack-quality-smoke` job summary and confirm it includes the scenario
  table plus the `codeinsight-context-pack-quality` artifact link.

  ```bash
  scripts/benchmark-artifact-smoke.sh <ci-run-id>
  scripts/context-pack-quality-artifact-smoke.sh <ci-run-id>
  ```

## Before Tagging A Release

- Use the recommended release path from [Maintenance commands](maintenance-commands.md)
  for the normal tag flow:

  ```bash
  scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp --evidence-file release-evidence/vX.Y.Z.md vX.Y.Z main
  scripts/prepare-release.sh --dry-run vX.Y.Z
  scripts/prepare-release.sh vX.Y.Z
  git push origin main
  scripts/release-pretag-check.sh main
  scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
  scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
  git tag -a vX.Y.Z -m "vX.Y.Z"
  git push origin vX.Y.Z
  ```

- Use [Release commands](release-commands.md) for the exact command index and
  [Release runbook](release-runbook.md) for troubleshooting, skip flags, and
  manual rebuild paths.

## After Publishing A Tag

- Watch the release workflows:

  ```bash
  gh run list --workflow "Release Build" --limit 5
  gh run list --workflow "Docker Image" --limit 5
  gh run watch <release-build-run-id> --exit-status
  gh run watch <docker-run-id> --exit-status
  ```

- Run the post-release verifier:

  ```bash
  scripts/post-release-verify.sh vX.Y.Z
  ```

- If Docker or Homebrew is unavailable on the local machine, skip only those
  local gates explicitly:

  ```bash
  scripts/post-release-verify.sh --skip-docker --skip-homebrew vX.Y.Z
  ```

- Confirm [Current status](status.md) contains the generated release
  verification summary for the released tag.

## When Investigating A User Report

- Reproduce with the installed binary when the report is about installation,
  MCP client setup, or quickstart behavior:

  ```bash
  CODEINSIGHT_BIN="$(command -v codeinsight)" scripts/installed-quickstart-smoke.sh
  ```

- Reproduce with local build/test fixtures when the report is about parsing,
  dependency resolution, context routing, or impact analysis:

  ```bash
  cargo test --locked
  scripts/mcp-stdio-smoke.sh
  scripts/two-minute-demo.sh
  scripts/agent-router-demo.sh
  ```

- Keep bug fixes scoped to the failing contract and add the smallest smoke or
  regression test that would have caught the issue.

## Related Docs

- [Release commands](release-commands.md)
- [Release runbook](release-runbook.md)
- [Release readiness](release-readiness.md)
- [Current status](status.md)
- [Demo script](demo-script.md)
- [MCP client smoke test](mcp-client-smoke.md)
- [Known limitations](known-limitations.md)
