# Release Commands

Use this as the short command index for release maintenance. For the full
step-by-step process, see [Release runbook](release-runbook.md).

## Short Path

Use this path when release metadata is ready and you are cutting a normal tag.
It keeps the operator-facing flow to three phases: dry-run evidence, prepare
and push metadata, then tag and verify.

```bash
# 1. Dry-run and archive pre-tag evidence.
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp --evidence-file release-evidence/vX.Y.Z.md --evidence-json-file release-evidence/vX.Y.Z.json vX.Y.Z main

# 2. Prepare and push the release metadata commit.
scripts/prepare-release.sh --dry-run vX.Y.Z
scripts/prepare-release.sh vX.Y.Z
git push origin main

# 3. Wait for CI, tag, then verify published artifacts.
scripts/release-pretag-check.sh main
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX.Y.Z.json vX.Y.Z main
scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
gh run list --workflow "Release Build" --limit 5
gh run list --workflow "Docker Image" --limit 5
gh run watch <release-build-run-id> --exit-status
gh run watch <docker-run-id> --exit-status
scripts/post-release-verify.sh --handoff vX.Y.Z
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
prepared metadata, prints the release evidence block, and ends with a
`release dry run checklist` covering the tag, commit, CI run, metadata fields,
and artifact gates without modifying the checkout.

To archive that evidence locally while still printing it to the terminal:

```bash
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp --evidence-file release-evidence/vX.Y.Z.md --evidence-json-file release-evidence/vX.Y.Z.json vX.Y.Z main
```

Add `--evidence-json-file release-evidence/vX.Y.Z.json` when another script or
automation needs the same evidence as structured JSON instead of parsing the
Markdown handoff block.

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
subset, context-pack quality, and agent-route artifacts:

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
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX.Y.Z.json vX.Y.Z main
```

This resolves the successful `CI` run for the target commit, validates the
`codeinsight-benchmark-subset`, `codeinsight-context-pack-quality`,
`codeinsight-agent-route-smoke`, and `codeinsight-mcp-first-call` artifacts,
and writes `release-evidence/vX.Y.Z.md` with the target SHA, CI run URL,
artifact URLs, local report paths, release metadata summary, and the
[CodeInsight self adoption report](adoption-report-codeinsight.md) command,
archive path, routed first-read metrics, and MCP first-call contract summary.
Use `--json-output PATH` to write the same evidence as machine-readable JSON,
`--output PATH` for a custom Markdown archive path, or `--force` to
intentionally overwrite an existing evidence file.

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
scripts/post-release-verify.sh --handoff vX.Y.Z
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
`release-verification/<tag>.json` by default, refreshes the generated
summary block in [Current status](status.md), and with `--handoff` also writes
`release-handoff/<tag>.json` and `release-handoff/<tag>.md`. When
`release-evidence/<tag>.json` exists, the status update also includes the
archived pre-release evidence fields from that machine-readable archive,
including the adoption report document, reproduce command, archive path,
routed first-read metric, and MCP first-call contract booleans when present. If
the JSON archive is missing, it falls back to `release-evidence/<tag>.md`. Use
`--evidence-json-file PATH` or `--evidence-file PATH` to pass a custom archive.
Use `--handoff-output PATH` or `--handoff-json-output PATH` to override the
handoff destinations.

## Targeted Checks

Run only published release verification:

```bash
scripts/verify-release.sh vX.Y.Z
scripts/verify-release.sh --json vX.Y.Z
```

Refresh status from an existing summary:

```bash
scripts/update-release-status.sh release-verification/vX.Y.Z.json
scripts/update-release-status.sh --evidence-json-file release-evidence/vX.Y.Z.json release-verification/vX.Y.Z.json
scripts/update-release-status.sh --evidence-file release-evidence/vX.Y.Z.md release-verification/vX.Y.Z.json
```

Build a release handoff summary from archived pre-release evidence and
post-release verification:

```bash
scripts/release-handoff-summary.sh --json-output release-handoff/vX.Y.Z.json --output release-handoff/vX.Y.Z.md vX.Y.Z
```

The handoff includes the adoption report document link, reproduce command,
`/tmp/codeinsight-self-adoption-report.tar.gz` archive path, `439/28433`
routed first-read metric, and MCP first-call contract booleans when
`release-evidence/<tag>.json` was generated by the current evidence script.

Build a release notes/status-PR draft from the release handoff JSON:

```bash
scripts/extract-release-notes.sh --summary --max-items 12 CHANGELOG.md vX.Y.Z /tmp/codeinsight-release-notes.md
scripts/release-notes-draft.sh --changelog-notes /tmp/codeinsight-release-notes.md --output release-handoff/vX.Y.Z.release-notes.md vX.Y.Z
```

Verify a local installed binary can complete the quickstart flow:

```bash
scripts/installed-quickstart-smoke.sh
```
