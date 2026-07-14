# Release Readiness

Use this checklist before a public tag, launch post, demo recording, or wider
MCP client adoption push. It answers one question: is this build ready to be
trusted by new users as an MVP?

For the mechanical release commands, use the [Release runbook](release-runbook.md).

## Current Baseline

The MVP is release-capable when all of these are true:

- The product story is centered on local-first AI-agent context routing.
- The install path works from a clean checkout and from published release
  artifacts.
- The first-read demo proves `index_project`, `project_overview`,
  `context_pack`, and `impact_analysis` work together.
- MCP clients can copy a working configuration and agent prompt.
- Benchmarks and limitations are visible before users over-trust the analyzer.
- CI and release workflows are green for the release commit or tag.

Current published baseline: `v0.1.11` is documented as verified in
[Current status](status.md). Treat later releases as unverified until this
checklist is repeated.

## Latest Release Verification

`v0.1.11` verification was completed on 2026-07-14:

- Release tag `v0.1.11` points to
  `fada3b030efa920fb0b7cabb55772f7ac93ae033`.
- `Release Build` run `29272402937` completed successfully.
- `Docker Image` run `29272402806` completed successfully.
- All four release archives returned HTTP 200:
  `codeinsight-aarch64-apple-darwin.tar.gz`,
  `codeinsight-x86_64-apple-darwin.tar.gz`,
  `codeinsight-aarch64-unknown-linux-gnu.tar.gz`, and
  `codeinsight-x86_64-unknown-linux-gnu.tar.gz`.
- Public installer path installed a binary reporting version `0.1.11`.
- GHCR image `ghcr.io/sleticalboy/codeinsight-mcp:0.1.11` exposes
  `linux/amd64` and `linux/arm64` manifests.
- Homebrew tap PR `sleticalboy/homebrew-tap#22` was merged, and tap `main`
  points to `4e9d032ab84aba359bb3ef41e035766cfe07632c`.

## Public MVP Gate

Before calling a build publicly ready, verify:

- [ ] `README.md` states the correct positioning: local-first MCP code context
  router for AI coding agents.
- [ ] [Quickstart](quickstart.md) gets a new user from install to MCP client
  usage without requiring hidden knowledge.
- [ ] [Install](install.md) includes current release installer, Homebrew,
  source, and Docker paths.
- [ ] [MCP client configuration](mcp-client-config.md) includes current Codex,
  Claude Code, Cursor, and generic JSON examples.
- [ ] [Agent prompt templates](agent-prompt-template.md) include copy-paste
  first-read, change-preflight, continuation, and review-planning prompts.
- [ ] [Demo script](demo-script.md) matches the real output shape of
  `scripts/agent-router-demo.sh`.
- [ ] [Known limitations](known-limitations.md) is linked from README and docs
  before any release announcement.

## Local Verification Gate

Run from the release candidate commit:

```bash
cargo fmt --check
cargo test --locked
scripts/agent-router-demo.sh
scripts/mcp-stdio-smoke.sh
scripts/semantic-smoke.sh
scripts/release-install-smoke.sh
git diff --check
```

Expected result:

- Rust unit and CLI integration tests pass.
- MCP stdio smoke reports `tools: 15` or the expected current tool count.
- Agent-router demo reports non-zero indexed files, non-zero symbols, four
  recommended next tools, a `context_pack` line-reduction value, and an
  `impact_analysis` summary.
- Semantic smoke completes with the deterministic local provider.
- Release install smoke proves the install script can install a packaged local
  artifact.
- `git diff --check` reports no whitespace errors.

Optional local checks when the environment supports them:

```bash
bash -n scripts/*.sh
scripts/docker-smoke.sh
CODEINSIGHT_DOCKER_PLATFORM=linux/arm64 scripts/docker-smoke.sh
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

## Fresh Checkout Rehearsal

Before the next public tag, rehearse from a clean checkout so cached build
state does not hide install or docs drift:

```bash
tmpdir="$(mktemp -d)"
git clone https://github.com/sleticalboy/CodeInsight-mcp.git "$tmpdir/CodeInsight-mcp"
cd "$tmpdir/CodeInsight-mcp"
cargo test --locked
cargo build --locked --release
scripts/agent-router-demo.sh
scripts/mcp-stdio-smoke.sh
scripts/release-install-smoke.sh
```

Accept the rehearsal only if the clean checkout requires no local-only files,
manual config, or undocumented credentials.

## Release Artifact Gate

For a tagged release, verify artifacts after the `Release Build` and
`Docker Image` workflows finish:

```bash
scripts/verify-release.sh vX.Y.Z
```

If Docker or Homebrew cannot run on the local machine, skip those local checks
explicitly:

```bash
CODEINSIGHT_SKIP_DOCKER=1 CODEINSIGHT_SKIP_HOMEBREW=1 scripts/verify-release.sh vX.Y.Z
```

`scripts/verify-release.sh` needs working GitHub CLI API access for release
metadata and remote tap checks. Authentication failures or API rate limits are
environment blockers; fix with `gh auth status` / `gh auth login`, or verify
the affected GitHub Actions and release pages manually before announcing.

The release is not complete until:

- [ ] GitHub Release exists and is not a draft.
- [ ] macOS and Linux archives exist for supported targets and their release
      download URLs are reachable.
- [ ] Public installer can install the requested version.
- [ ] Homebrew formula is updated or an open tap PR exists.
- [ ] GHCR image is published for `linux/amd64` and `linux/arm64`.
- [ ] Release notes are scoped to the current version, not the whole changelog.

## CI Gate

Required workflows:

- `CI`
- `Release Build` for tags or manual release-sync runs
- `Docker Image` for tags

Use:

```bash
gh run list --branch main --limit 5
gh run list --workflow "Release Build" --limit 5
gh run list --workflow "Docker Image" --limit 5
```

If GitHub API rate limits block `gh`, record that as an environment blocker and
confirm the runs in the GitHub Actions UI before tagging or announcing.

## Benchmark Evidence Gate

Refresh benchmark evidence when `context_pack`, ranking, continuation, import
resolution, or entrypoint detection changes:

```bash
scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

Update these docs if the output changes materially:

- [Smoke benchmark](benchmark-v0.1.md)
- [Large repository benchmark](benchmark-large.md)
- README benchmark snapshot

The benchmark story should support the product claim that CodeInsight routes an
agent to bounded local context. Do not present the fixture benchmarks as
controlled performance benchmarks.

## Communication Gate

Before publishing a release announcement, make sure the message says:

- CodeInsight is local-first.
- The strongest use case is AI-agent first-read routing.
- `context_pack` reduces blind repository reading by selecting bounded files
  and ranges.
- `impact_analysis` is a pre-edit planning aid.
- Call graphs, references, and impact analysis are best-effort navigation
  evidence, not compiler-grade proof.

Avoid claiming:

- Full IDE or LSP replacement.
- Compiler-grade static analysis.
- Default semantic search quality without a configured embedding provider.
- Enterprise team workflows that are not implemented yet.

## Go / No-Go Summary

Use this final summary before tagging:

- [ ] Product positioning is clear and narrow.
- [ ] Local verification gate passed.
- [ ] Fresh checkout rehearsal passed.
- [ ] CI is green or explicitly blocked by a known external issue.
- [ ] Release artifacts are verified for tagged releases.
- [ ] Benchmark evidence is current for the changed behavior.
- [ ] Known limitations are visible from the README path.
- [ ] Next release notes are ready in `CHANGELOG.md`.

If any item fails, do not tag or announce. Fix the blocker, then repeat the
smallest affected gate.
