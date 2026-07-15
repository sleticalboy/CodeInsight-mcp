# Release Commands

Use this as the short command index for release maintenance. For the full
step-by-step process, see [Release runbook](release-runbook.md).

## Short Path

Use this path when release metadata is ready and you are cutting a normal tag:

```bash
scripts/prepare-release.sh --dry-run vX.Y.Z
scripts/prepare-release.sh vX.Y.Z
git push origin main
scripts/release-pretag-check.sh main
scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
gh run list --workflow "Release Build" --limit 5
gh run list --workflow "Docker Image" --limit 5
gh run watch <release-build-run-id> --exit-status
gh run watch <docker-run-id> --exit-status
scripts/post-release-verify.sh vX.Y.Z
```

If Docker or Homebrew cannot run locally, use the explicit post-release skip
flags documented below and verify those gates through GitHub Actions, GHCR, or
the Homebrew tap.

## Before Tagging

Preview release metadata changes:

```bash
scripts/prepare-release.sh --dry-run vX.Y.Z
```

Prepare the release commit:

```bash
scripts/prepare-release.sh vX.Y.Z
```

Run the local release gate:

```bash
scripts/local-ci-smoke.sh
scripts/semantic-smoke.sh
scripts/mcp-stdio-smoke.sh
scripts/release-install-smoke.sh
scripts/installed-quickstart-smoke.sh
```

After the release prep commit CI completes, validate the uploaded benchmark
subset artifact:

```bash
scripts/release-pretag-check.sh main
```

Dry-run the tag release path without creating or pushing a tag:

```bash
scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
```

Tagged `Release Build` runs also execute this gate against the tag target SHA
before building release artifacts:

```bash
scripts/release-pretag-check.sh --repo sleticalboy/CodeInsight-mcp --head-sha <tag-target-sha> main
```

## Publish

Push the release prep commit, then create and push the tag:

```bash
git push origin main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

Watch the release workflows:

```bash
gh run list --workflow "Release Build" --limit 5
gh run list --workflow "Docker Image" --limit 5
gh run watch <release-build-run-id> --exit-status
gh run watch <docker-run-id> --exit-status
```

## After Release

Run the recommended post-release verification command:

```bash
scripts/post-release-verify.sh vX.Y.Z
```

If Docker or Homebrew is not usable on the local machine, skip only those local
gates explicitly:

```bash
scripts/post-release-verify.sh --skip-docker --skip-homebrew vX.Y.Z
```

If direct `github.com/releases/download` asset URLs are blocked from the local
network but GitHub API metadata confirms the assets, continue with
metadata-only asset verification:

```bash
scripts/post-release-verify.sh --allow-asset-download-unreachable vX.Y.Z
```

The post-release verifier saves a JSON summary under
`release-verification/<tag>.json` by default and refreshes the generated
summary block in [Current status](status.md).

## Targeted Checks

Run only published release verification:

```bash
scripts/verify-release.sh vX.Y.Z
scripts/verify-release.sh --json vX.Y.Z
```

Refresh status from an existing summary:

```bash
scripts/update-release-status.sh release-verification/vX.Y.Z.json
```

Verify a local installed binary can complete the quickstart flow:

```bash
scripts/installed-quickstart-smoke.sh
```
