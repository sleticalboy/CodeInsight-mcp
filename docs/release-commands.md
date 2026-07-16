# Release Commands

Use this as the short command index for release maintenance. For the full
step-by-step process, see [Release runbook](release-runbook.md).

## Short Path

Use this path when release metadata is ready and you are cutting a normal tag.
It keeps the operator-facing flow to three phases: dry-run evidence, prepare
and push metadata, then tag and verify.

```bash
# 1. Dry-run and archive pre-tag evidence.
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp --evidence-file release-evidence/vX.Y.Z.md vX.Y.Z main

# 2. Prepare and push the release metadata commit.
scripts/prepare-release.sh --dry-run vX.Y.Z
scripts/prepare-release.sh vX.Y.Z
git push origin main

# 3. Wait for CI, tag, then verify published artifacts.
scripts/release-pretag-check.sh main
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
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

Run the full pre-release dry run:

```bash
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
```

This prints the release prep diff, validates tag preflight with temporary
prepared metadata, and prints the release evidence block without modifying the
checkout.

To archive that evidence locally while still printing it to the terminal:

```bash
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp --evidence-file release-evidence/vX.Y.Z.md vX.Y.Z main
```

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
subset and context-pack quality artifacts:

```bash
scripts/release-pretag-check.sh main
```

Dry-run the tag release path without creating or pushing a tag:

```bash
scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
```

This fails if the tag already exists locally, the remote tag already exists, or
a GitHub Release already exists for the tag. It also verifies that
`Cargo.toml`, `docs/install.md`, and `CHANGELOG.md` are prepared for the same
version and prints `metadata_cargo`, `metadata_install`, and
`metadata_changelog` summary lines.

Archive the release evidence block for the release handoff:

```bash
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
```

This resolves the successful `CI` run for the target commit, validates the
`codeinsight-benchmark-subset` and `codeinsight-context-pack-quality`
artifacts, and writes `release-evidence/vX.Y.Z.md` with the target SHA, CI run
URL, benchmark artifact URL, context-pack quality artifact URL, local report
paths, and release metadata summary. Use `--output PATH` for a custom archive
path or `--force` to intentionally overwrite an existing evidence file.

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
