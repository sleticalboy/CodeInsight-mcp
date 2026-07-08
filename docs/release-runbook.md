# Release Runbook

This document is the operational checklist for publishing CodeInsight releases.

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

## Prepare A Release

Update these files before tagging:

- `Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`
- version examples in `README.md`, if they mention a specific release

Local release gate:

```bash
cargo check
cargo fmt --check
cargo test --locked
bash -n scripts/*.sh
scripts/semantic-smoke.sh
scripts/mcp-stdio-smoke.sh
scripts/release-install-smoke.sh
```

Commit and push the release prep:

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md README.md
git commit -m "chore: prepare vX.Y.Z release"
git push origin main
```

Wait for CI:

```bash
gh run list --branch main --limit 5
gh run watch <run-id> --exit-status
```

## Publish A Tagged Release

Create and push an annotated tag:

```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

The tag triggers:

- `Release Build`: Linux and macOS release assets, GitHub Release notes, Homebrew tap sync.
- `Docker Image`: GHCR image publish.

Watch the release workflows:

```bash
gh run list --workflow "Release Build" --limit 5
gh run list --workflow "Docker Image" --limit 5
gh run watch <release-build-run-id> --exit-status
gh run watch <docker-run-id> --exit-status
```

## Verify Release Assets

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

The generated notes must not include older release sections.

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
scripts/extract-release-notes.sh CHANGELOG.md vX.Y.Z /tmp/codeinsight-release-notes.md
gh release edit vX.Y.Z --notes-file /tmp/codeinsight-release-notes.md
```
