# Maintainer Checklist

Use this checklist for routine CodeInsight maintenance. It links to the deeper
docs when a task needs more detail.

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
  cargo fmt --check
  cargo test --locked
  bash -n scripts/*.sh
  scripts/docs-smoke.sh
  git diff --check
  ```

- For changes that affect first-read routing, context packing, MCP output, or
  install behavior, also run the relevant smoke:

  ```bash
  scripts/agent-router-demo.sh
  scripts/mcp-stdio-smoke.sh
  scripts/semantic-smoke.sh
  scripts/installed-quickstart-smoke.sh
  ```

- For release-tooling changes, run the release script smokes:

  ```bash
  scripts/install-fallback-smoke.sh
  scripts/verify-release-summary-smoke.sh
  scripts/post-release-verify-smoke.sh
  scripts/update-release-status-smoke.sh
  ```

## Before Tagging A Release

- Use the short command index for the exact release commands:
  [Release commands](release-commands.md).

- Preview release metadata updates:

  ```bash
  scripts/prepare-release.sh --dry-run vX.Y.Z
  ```

- Prepare the release commit:

  ```bash
  scripts/prepare-release.sh vX.Y.Z
  ```

- Run the local release gate from [Release commands](release-commands.md)
  before pushing the release prep commit.

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
  scripts/agent-router-demo.sh
  ```

- Keep bug fixes scoped to the failing contract and add the smallest smoke or
  regression test that would have caught the issue.

## Related Docs

- [Release commands](release-commands.md)
- [Release runbook](release-runbook.md)
- [Current status](status.md)
- [MCP client smoke test](mcp-client-smoke.md)
- [Known limitations](known-limitations.md)
