# Release Readiness

Use this checklist before a public tag, launch post, demo recording, or wider
MCP client adoption push. It answers one question: is this build ready to be
trusted by new users as an MVP?

For the mechanical release commands, use [Release commands](release-commands.md)
or the full [Release runbook](release-runbook.md).

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

Current published baseline: `v0.1.12` is documented as verified in
[Current status](status.md). Treat later releases as unverified until this
checklist is repeated.

## Latest Release Verification

`v0.1.12` verification was completed on 2026-07-14:

- Release tag `v0.1.12` points to
  `43010eba5683d45148fb113d743461917c6acb91`.
- `Release Build` run `29310231429` completed successfully.
- `Docker Image` run `29310231461` completed successfully.
- All four release archives exist with GitHub API digests:
  `codeinsight-aarch64-apple-darwin.tar.gz`,
  `codeinsight-x86_64-apple-darwin.tar.gz`,
  `codeinsight-aarch64-unknown-linux-gnu.tar.gz`, and
  `codeinsight-x86_64-unknown-linux-gnu.tar.gz`.
- Public installer path was covered by the release-install smoke test for the
  `v0.1.12` candidate.
- GHCR image `ghcr.io/sleticalboy/codeinsight-mcp:0.1.12` exposes
  `linux/amd64` and `linux/arm64` manifests, with index digest
  `sha256:09238cfbca454e94cc04b4006f4dd1220619b640177f23c41cb7d819469ed2df`.
- Homebrew tap PR `sleticalboy/homebrew-tap#23` was merged, and tap `main`
  points to `dc691f27b6279141232d845bd1b833d25ab0f148`.

Local caveat: direct `curl` checks against `github.com/releases/download/...`
and local Homebrew tap `git fetch` timed out on the current development
machine. GitHub API release metadata, GitHub Actions, GHCR manifest inspection,
and the remote Homebrew formula were reachable.

## Current Release Tooling Baseline

`main` has a stricter verification path for tagged releases:

- Public installer verification tolerates a broken local `gh` install by
  falling back to `curl`.
- GitHub Release archive checks include direct URL reachability, with ranged
  `GET` fallback when `HEAD` is rejected.
- GitHub Release archive failures explain the difference between missing assets
  and a local `github.com/releases/download` connectivity problem, and expose a
  metadata-only override for the latter.
- GitHub CLI, Docker, and Homebrew failures include tool-specific recovery
  guidance.
- Docker verification separates local daemon/Buildx problems from registry,
  manifest, and platform problems.
- Homebrew verification separates dirty local taps, formula version mismatch,
  remote tap state, and fetch/checksum failures.
- `scripts/verify-release.sh --json vX.Y.Z` emits a final summary after all
  enabled gates pass.

Before the next public tag, rerun the artifact gate from an environment with
working GitHub CLI auth. If local Docker or Homebrew are unavailable, use the
explicit skip variables and verify those gates through the corresponding
GitHub Actions workflow, GHCR tag, or remote tap formula.

`scripts/prepare-release.sh --dry-run v0.1.12` was rehearsed on 2026-07-14 and
previewed the expected Cargo version, CHANGELOG `0.1.12` section, and
`docs/install.md` version-pin update before the release commit was created.

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
- [ ] README benchmark snapshot, [Demo script](demo-script.md), and benchmark
  `Key Results` tell the same routing and compression story.
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
scripts/installed-quickstart-smoke.sh
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
- Installed quickstart smoke proves an installed `codeinsight` binary can
  complete `version`, `index`, `overview`, `context-pack`, and MCP stdio calls
  against a temporary project outside the source checkout.
- `git diff --check` reports no whitespace errors.

Optional local checks when the environment supports them:

```bash
scripts/script-syntax-smoke.sh
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
scripts/installed-quickstart-smoke.sh
```

Accept the rehearsal only if the clean checkout requires no local-only files,
manual config, or undocumented credentials.

## Release Artifact Gate

For a tagged release, verify artifacts after the `Release Build` and
`Docker Image` workflows finish:

```bash
scripts/verify-release.sh vX.Y.Z
```

Use `scripts/verify-release.sh --json vX.Y.Z` when the final pass/fail summary
needs to be copied into CI logs, release notes, or the verification record.
The consolidated script also runs the installed quickstart smoke against the
binary installed by the public install script.
Use `scripts/post-release-verify.sh vX.Y.Z` after release workflows finish to
run verification, save the JSON summary, and refresh the generated verification
summary in [Current status](status.md).

If Docker or Homebrew cannot run on the local machine, skip those local checks
explicitly:

```bash
CODEINSIGHT_SKIP_DOCKER=1 CODEINSIGHT_SKIP_HOMEBREW=1 scripts/verify-release.sh vX.Y.Z
```

If the local machine cannot run the installed quickstart smoke because of a
temporary Python or shell environment issue, skip that gate explicitly with
`CODEINSIGHT_SKIP_INSTALLED_QUICKSTART=1` and run
`scripts/installed-quickstart-smoke.sh` on a suitable machine before
announcing.

When Docker verification is enabled, local Docker daemon and Buildx failures are
environment blockers. If they cannot be fixed locally, skip Docker verification
with `CODEINSIGHT_SKIP_DOCKER=1` and confirm the `Docker Image` workflow result
plus GHCR tags before announcing.

When Homebrew verification is enabled, dirty local tap checkouts, stable version
mismatches, and `brew fetch` checksum/download failures are blockers. If the
local Homebrew environment is the only problem, skip with
`CODEINSIGHT_SKIP_HOMEBREW=1` and confirm the remote tap formula separately.

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
- [Demo script](demo-script.md) evidence cutaway

The benchmark story should support the product claim that CodeInsight routes an
agent to bounded local context. The README benchmark snapshot, demo evidence
cutaway, and report `Key Results` should agree on `context_pack` first-tool
routing, aggregate line reduction, and benchmark scope. Do not present the
fixture benchmarks as controlled performance benchmarks.

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
