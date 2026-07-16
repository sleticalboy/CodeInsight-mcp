# Release Runbook

This document is the operational checklist for publishing CodeInsight releases.
For the short command index, see [Release commands](release-commands.md).

## Prerequisites

- GitHub CLI authenticated with access to `sleticalboy/CodeInsight-mcp`.
- `repo` and `workflow` scopes for release and Actions operations.
- `HOMEBREW_TAP_TOKEN` configured as an Actions secret on `sleticalboy/CodeInsight-mcp`.
- The token must be able to read and push `sleticalboy/homebrew-tap`.

Check the current state:

```bash
gh auth status
gh secret list --repo sleticalboy/CodeInsight-mcp
git status --short --branch
```

Set or rotate the Homebrew tap token:

```bash
gh auth token | gh secret set HOMEBREW_TAP_TOKEN --repo sleticalboy/CodeInsight-mcp
```

## Recommended SOP

Normal releases should follow this three-phase flow:

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
scripts/post-release-verify.sh vX.Y.Z
```

The sections below document the same flow with troubleshooting details and
optional verification commands.

## Prepare A Release

Run the full pre-release dry run:

```bash
scripts/release-dry-run.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
```

This command previews the release metadata diff, applies that metadata in a
temporary copy, runs the tag preflight against the target commit, and prints the
release evidence block. It does not modify the checkout.

Use `--evidence-file release-evidence/vX.Y.Z.md` when you want to archive the
pre-tag evidence block for handoff or release review.

Prepare release metadata:

```bash
scripts/prepare-release.sh vX.Y.Z
```

Preview the generated changes without editing the workspace:

```bash
scripts/prepare-release.sh --dry-run vX.Y.Z
```

The script updates:

- `Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`
- version examples in `docs/install.md`

The target version must be greater than the current `Cargo.toml` package
version. If you intentionally need to cut a metadata-only release while
`Unreleased` is empty, set `CODEINSIGHT_ALLOW_EMPTY_CHANGELOG=1`.

Local release gate:

```bash
cargo check
cargo fmt --check
cargo test --locked
scripts/script-syntax-smoke.sh
scripts/semantic-smoke.sh
scripts/mcp-stdio-smoke.sh
scripts/release-install-smoke.sh
scripts/installed-quickstart-smoke.sh
```

Commit and push the release prep:

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md docs/install.md
git commit -m "chore: prepare vX.Y.Z release"
git push origin main
```

Wait for CI:

```bash
scripts/release-pretag-check.sh main
```

This waits for the latest `CI` run on `main`, downloads
`codeinsight-benchmark-subset` and `codeinsight-context-pack-quality`,
validates both artifacts, and confirms the benchmark plus context-pack quality
evidence is readable outside the GitHub Actions UI before tagging.

Dry-run the tag preflight without creating or pushing a tag:

```bash
scripts/release-tag-preflight.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
```

This validates the tagged `Release Build` workflow guard and re-checks the tag
target commit by SHA against the successful `CI` run, benchmark artifact, and
context-pack quality artifact. It also fails fast when the tag already exists
locally or remotely, or when a GitHub Release already exists for the tag. The
target tag must match `Cargo.toml`, the pinned installer example in
`docs/install.md`, and the prepared `CHANGELOG.md` release section. The
preflight output prints `metadata_cargo`, `metadata_install`, and `metadata_changelog`
so the prepared versions are visible before tagging. It also prints
`artifact_gate_benchmark: passed` and
`artifact_gate_context_pack_quality: passed` after the CI artifact gates pass.

Archive the release evidence block:

```bash
scripts/archive-release-evidence.sh --repo sleticalboy/CodeInsight-mcp vX.Y.Z main
```

This resolves the successful `CI` run for the target SHA, validates the
`codeinsight-benchmark-subset` and `codeinsight-context-pack-quality`
artifacts, and prints a Markdown block with the commit, workflow run,
benchmark artifact, context-pack quality artifact, local report paths, and
release metadata to `release-evidence/vX.Y.Z.md`. Use `--output PATH` for a
custom archive path or `--force` to intentionally overwrite an existing
evidence file.

## Publish A Tagged Release

Create and push an annotated tag:

```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

The tag triggers:

- `Release Build`: Linux and macOS release assets, GitHub Release notes, Homebrew tap sync.
- `Docker Image`: GHCR image publish.

The tagged `Release Build` workflow first runs `verify-pretag-ci`, which checks
that the tag target SHA has a successful `CI` run on `main` and validates that
same run's `codeinsight-benchmark-subset` and
`codeinsight-context-pack-quality` artifacts before any release assets are
built.

Watch the release workflows:

```bash
gh run list --workflow "Release Build" --limit 5
gh run list --workflow "Docker Image" --limit 5
gh run watch <release-build-run-id> --exit-status
gh run watch <docker-run-id> --exit-status
```

## Verify Release Assets

Run the consolidated release verification script after the release and Docker
workflows finish:

```bash
scripts/verify-release.sh vX.Y.Z
```

Run the post-release verifier to save a machine-readable summary and refresh
the generated status summary:

```bash
scripts/post-release-verify.sh vX.Y.Z
```

If the local machine cannot run Docker or Homebrew, pass the same explicit
skip options:

```bash
scripts/post-release-verify.sh --skip-docker --skip-homebrew vX.Y.Z
```

The consolidated script installs the tagged binary with the public installer,
then runs `scripts/installed-quickstart-smoke.sh` against that installed
binary. This confirms a new user can complete the quickstart CLI flow and MCP
stdio calls against a temporary project outside the source checkout.

The GitHub Release step validates both metadata and direct downloadability for
all four platform archives. It first tries HTTP `HEAD` for each release asset
URL, then retries with a ranged `GET` if the server or proxy rejects `HEAD`.

If GitHub API metadata is reachable but `github.com/releases/download/...`
times out from the local machine, the script reports that as a local
network/proxy path issue. Keep the default strict check for final release
signoff when possible. To continue collecting the remaining Docker and Homebrew
evidence from that machine, use the explicit metadata-only override:

```bash
CODEINSIGHT_ALLOW_ASSET_DOWNLOAD_UNREACHABLE=1 scripts/verify-release.sh vX.Y.Z
```

The JSON summary marks this as `github_asset_downloads: "metadata_only"` so it
is not confused with a full direct-download pass.

The same override is available through the post-release wrapper:

```bash
scripts/post-release-verify.sh --allow-asset-download-unreachable vX.Y.Z
```

If the local machine cannot run Docker or Homebrew, skip those checks
explicitly and rely on the successful GitHub Actions jobs plus the remaining
remote checks:

```bash
CODEINSIGHT_SKIP_DOCKER=1 CODEINSIGHT_SKIP_HOMEBREW=1 scripts/verify-release.sh vX.Y.Z
```

If the release machine cannot run the installed quickstart smoke because of a
temporary Python or shell environment issue, skip only that gate explicitly:

```bash
CODEINSIGHT_SKIP_INSTALLED_QUICKSTART=1 scripts/verify-release.sh vX.Y.Z
```

Run `scripts/installed-quickstart-smoke.sh` on a suitable machine before
publishing the release announcement.

When Docker verification is enabled, `scripts/verify-release.sh` first checks
Docker daemon and Buildx availability, then validates the GHCR manifest digest,
`linux/amd64` and `linux/arm64` platforms, and container `version` output. If
Docker is not usable on the local machine, treat that as an environment blocker
or rerun with `CODEINSIGHT_SKIP_DOCKER=1` and confirm the Docker Image workflow
separately.

When Homebrew verification is enabled, the script checks the remote formula,
refreshes the local tap when possible, verifies the stable version, and runs
`brew fetch` to validate the archive checksums. Dirty local tap checkouts,
formula version mismatches, and checksum/fetch failures are blockers. If only
local Homebrew state is broken, rerun with `CODEINSIGHT_SKIP_HOMEBREW=1` and
confirm the remote tap PR or formula manually.

`scripts/verify-release.sh` still requires usable GitHub CLI API access for
release metadata and Homebrew tap checks. If it reports authentication or rate
limit errors, run:

```bash
gh auth status
gh auth login
```

Then rerun the verification command. The installer check itself can still fall
back from a broken `gh release download` path to `curl`, but release metadata
verification remains a GitHub API check.

Check release metadata:

```bash
gh release view vX.Y.Z --json tagName,url,isDraft,isPrerelease,publishedAt \
  --jq '{tagName,url,isDraft,isPrerelease,publishedAt}'
```

Check release assets:

```bash
gh release view vX.Y.Z --json assets \
  --jq '.assets[] | {name,size,downloadCount}'
```

Expected assets:

- `codeinsight-aarch64-apple-darwin.tar.gz`
- `codeinsight-aarch64-unknown-linux-gnu.tar.gz`
- `codeinsight-x86_64-apple-darwin.tar.gz`
- `codeinsight-x86_64-unknown-linux-gnu.tar.gz`

Smoke test the public installer path:

```bash
tmpdir="$(mktemp -d)"
INSTALL_DIR="$tmpdir/bin" CODEINSIGHT_VERSION=vX.Y.Z sh scripts/install.sh
"$tmpdir/bin/codeinsight" --help | sed -n '1,12p'
rm -rf "$tmpdir"
```

If GitHub CLI download hangs locally, force the `curl` branch:

```bash
tmpdir="$(mktemp -d)"
env PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
  INSTALL_DIR="$tmpdir/bin" \
  CODEINSIGHT_VERSION=vX.Y.Z \
  sh scripts/install.sh
"$tmpdir/bin/codeinsight" --help | sed -n '1,12p'
rm -rf "$tmpdir"
```

## Verify Homebrew

The release workflow updates the shared tap when `HOMEBREW_TAP_TOKEN` is set.

Manual sync for an existing release tag:

```bash
gh workflow run "Release Build" --ref main -f tag=vX.Y.Z
gh run list --workflow "Release Build" --limit 5
gh run watch <run-id> --exit-status
```

Expected manual-sync behavior:

- `build` is skipped.
- `publish-github-release` is skipped.
- `sync-homebrew-tap` downloads existing release assets.
- The tap update either creates/updates a PR or prints `Homebrew tap formula is already up to date.`

Verify the tap state:

```bash
git -C /Users/binlee/code/open-source/homebrew-tap ls-remote origin refs/heads/main
brew tap sleticalboy/tap
brew info sleticalboy/tap/codeinsight
```

Formula checks:

```bash
ruby -c Formula/codeinsight.rb
brew style Formula/codeinsight.rb
```

Full Homebrew audit/install checks may require Homebrew to fetch `homebrew/core`
and package metadata. If the local network is unstable, treat that as an
environment blocker and rely on the GitHub Actions tap-sync result plus formula
syntax/style checks.

## Verify Docker

Tagged releases publish the image to GHCR for `linux/amd64` and `linux/arm64`.

Check the workflow:

```bash
gh run list --workflow "Docker Image" --limit 5
gh run watch <run-id> --exit-status
```

Smoke test a local Docker build when Docker is available:

```bash
scripts/docker-smoke.sh
CODEINSIGHT_DOCKER_PLATFORM=linux/arm64 scripts/docker-smoke.sh
```

## Release Notes

Release notes are generated from the matching `CHANGELOG.md` section:

```bash
scripts/extract-release-notes.sh CHANGELOG.md vX.Y.Z /tmp/codeinsight-release-notes.md
sed -n '1,80p' /tmp/codeinsight-release-notes.md
```

The generated notes must not include older release sections. Use the compact
summary form for the public GitHub Release page when a changelog section is
large:

```bash
scripts/extract-release-notes.sh --summary --max-items 12 CHANGELOG.md vX.Y.Z /tmp/codeinsight-release-notes.md
```

CI validates the first versioned changelog section automatically:

```bash
scripts/latest-changelog-version.sh CHANGELOG.md
scripts/extract-release-notes.sh CHANGELOG.md latest /tmp/codeinsight-release-notes.md
```

## Recovery

Re-run a release build for an existing tag:

```bash
gh workflow run "Release Build" --ref vX.Y.Z
```

Re-sync only Homebrew for an existing tag:

```bash
gh workflow run "Release Build" --ref main -f tag=vX.Y.Z
```

Re-upload release assets after a successful rebuild:

```bash
gh release upload vX.Y.Z release-assets/*.tar.gz --clobber
```

Update release notes only:

```bash
scripts/extract-release-notes.sh --summary --max-items 12 CHANGELOG.md vX.Y.Z /tmp/codeinsight-release-notes.md
gh release edit vX.Y.Z --notes-file /tmp/codeinsight-release-notes.md
```
