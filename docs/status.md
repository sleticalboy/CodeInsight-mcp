# Current Status

CodeInsight is a local-first MVP code-intelligence server. The current build is
useful for repository indexing, navigation, dependency/call graph inspection,
impact triage, semantic-context experiments, and AI-agent context gathering. It
is not a complete language-server-grade static-analysis platform, but the core
MVP workflow is now implemented end to end.

## Implemented

- Rust CLI entrypoint.
- Local SQLite index cache under `.codeinsight/`.
- Incremental indexing with file-hash skips and stale file cleanup.
- Index metadata with schema and index version tracking.
- Per-file indexing errors in reports without aborting the whole project scan.
- Tree-sitter parsing for TypeScript/JavaScript, Python, Go, Rust, Java, C,
  C++, C#, PHP, and Ruby.
- Symbol extraction for common declarations.
- Repository overview with dependency/call summaries, role-aware directories,
  entrypoint candidates, and MCP-ready recommended next tools.
- Framework-oriented entrypoint signals for common first-read surfaces,
  including Next.js app router files, Next.js pages bootstrap files, Rails
  routes, Java application roots, Python web framework roots, and C# web
  application roots.
- Dependency graph with local resolution, source/target file filters, language
  filters, summaries, top source stats, and top target stats.
- Text reference search, impact analysis, token-budgeted context packs, reading
  plans, and call graph tools with imported target hints.
- Local dependency resolution for common import/include/use forms across the
  supported languages, including JavaScript/TypeScript package metadata,
  workspaces, Python relative imports, Rust modules, Go modules, Java/C#/PHP
  namespace imports, Ruby `require_relative`, and C/C++ local includes.
- Same-file `callee_file` hints for local calls that match symbols in the
  current file, plus imported `callee_file` hints for obvious local calls in
  JavaScript/TypeScript, Python, Rust, Go, Java, C#, PHP, and Ruby.
- Embedding provider interface, provider status reporting, and local semantic
  search paths over local vectors.
- Local semantic chunk index storage with optional deterministic local-hash
  embedding generation.
- `context_pack` semantic status, reading plan, candidate `selection_rank`,
  raw `selection_reason`, and file-scoped follow-up tool suggestions for
  `file_outline`, `impact_analysis`, `dependency_graph`, and focused
  `context_pack` calls.
- `context_pack` budget metadata, bounded omitted-candidate follow-ups, and a
  `continuation_summary.next_action` that lets MCP clients expose a single next
  action after the initial reading plan.
- CLI commands: `index`, `init-config`, `config-status`, `overview`,
  `symbols`, `outline`, `dependency-graph`, `impact-analysis`,
  `find-references`, `semantic-search`, `semantic-index`, `embedding-status`,
  `context-pack`, `agent-route`, `callers`, and `callees`.
- MCP stdio `initialize`, `tools/list`, and `tools/call`, including the
  one-call `agent_route` first-read path.
- MCP tool argument validation with stable JSON-RPC errors.
- Fixture-based CLI and MCP stdio integration tests.
- Local smoke scripts for the one-call `agent_route` contract, the
  agent-router first-read demo, MCP stdio, semantic search, Docker, release
  install, and benchmark fixtures.
- CI evidence artifacts for benchmark, context-pack quality, the one-call
  `agent_route` contract, and MCP first-call onboarding, each with an Actions
  summary and release evidence validation path for quick inspection.
- Release, Docker image, Homebrew tap sync, install, verify, and release-note
  helper scripts.
- Published and verified `v0.1.12` with GitHub Release assets, Docker
  multi-arch images, public install script, and Homebrew tap formula.

## Latest Verified Release

`v0.1.12` was published and verified on 2026-07-14.

- GitHub Release: https://github.com/sleticalboy/CodeInsight-mcp/releases/tag/v0.1.12
- Release tag `v0.1.12` points to
  `43010eba5683d45148fb113d743461917c6acb91`.
- Release Build workflow: `29310231429`, completed successfully.
- Docker Image workflow: `29310231461`, completed successfully.
- Release assets exist for macOS and Linux on `aarch64` and `x86_64`, with
  GitHub API digests matching the Homebrew formula:
  - `codeinsight-aarch64-apple-darwin.tar.gz`:
    `sha256:2b6042662001f213ad8042960f8dd8be13880efde32b05c830856fa41bf8a230`
  - `codeinsight-x86_64-apple-darwin.tar.gz`:
    `sha256:9fd614e77c8aa5729ec6b9e8c615526f43596d00f8ba3618e2c92ccf48b27a24`
  - `codeinsight-aarch64-unknown-linux-gnu.tar.gz`:
    `sha256:52bdef03b67f3e00e4b9a89a7dca1cb2f0515e58adefd1cffc1ec5120b631406`
  - `codeinsight-x86_64-unknown-linux-gnu.tar.gz`:
    `sha256:ab7919ba44780d97389368186e1afc68c272828df843bd55a18592217c2ed1df`
- Public installer path was verified with the remote install command and
  `CODEINSIGHT_VERSION=v0.1.12`.
- Docker image verified at `ghcr.io/sleticalboy/codeinsight-mcp:0.1.12`
  with digest
  `sha256:09238cfbca454e94cc04b4006f4dd1220619b640177f23c41cb7d819469ed2df`
  and `linux/amd64` and `linux/arm64` manifests.
- Homebrew tap PR `sleticalboy/homebrew-tap#23` was merged, and tap `main`
  points to `dc691f27b6279141232d845bd1b833d25ab0f148`.
- Post-release install regression on 2026-07-14 verified the remote installer,
  Homebrew `brew install sleticalboy/tap/codeinsight`, and GHCR `latest` /
  `0.1.12` manifest metadata.
- Installed-binary quickstart regression on 2026-07-14 verified
  `/opt/homebrew/bin/codeinsight` can run `version`, `index`, `overview`,
  `context-pack`, and MCP stdio calls against a temporary project outside this
  source checkout. Current installed quickstart checks also cover CLI
  `agent-route` and MCP `agent_route`.
- Consolidated release verification passed with
  `CODEINSIGHT_SKIP_DOCKER=1 scripts/verify-release.sh --json v0.1.12`:
  GitHub Release metadata, direct asset downloads, release notes, public
  installer, remote Homebrew formula, and Homebrew fetch all passed.


<!-- release-verification-summary:start -->
### Release Verification Summary

Generated from `scripts/verify-release.sh --json` on 2026-07-14.

- Status: `passed`
- Tag: `v0.1.12`
- Version: `0.1.12`
- Repository: `sleticalboy/CodeInsight-mcp`
- Gates:
  - `github_release`: `passed`
  - `github_asset_downloads`: `passed`
  - `release_notes`: `passed`
  - `install_script`: `passed`
  - `installed_quickstart`: `passed`
  - `docker`: `skipped`
  - `homebrew_remote_formula`: `passed`
  - `homebrew_fetch`: `passed`
- Expected release assets:
  - `codeinsight-aarch64-apple-darwin.tar.gz`
  - `codeinsight-aarch64-unknown-linux-gnu.tar.gz`
  - `codeinsight-x86_64-apple-darwin.tar.gz`
  - `codeinsight-x86_64-unknown-linux-gnu.tar.gz`
- Docker image: `ghcr.io/sleticalboy/codeinsight-mcp` (skipped locally)
- Homebrew tap: `sleticalboy/tap` (verified)
- Installed quickstart binary: `/opt/homebrew/bin/codeinsight` (verified)
<!-- release-verification-summary:end -->

## Current Release Tooling State

`main` includes the release-verification hardening used during the `v0.1.12`
release:

- `scripts/install.sh` falls back from a failing `gh release download` path to
  `curl` for public release assets.
- `scripts/install.sh` bounds GitHub CLI and curl release downloads with
  explicit timeouts so installer verification cannot hang indefinitely on a
  broken local network path.
- `scripts/verify-release.sh` checks direct HTTP downloadability for all four
  expected release archives.
- Asset download failures now distinguish missing release assets from local
  `github.com/releases/download` reachability problems, with an explicit
  metadata-only override when the release API already confirms the assets.
- GitHub CLI, Docker, and Homebrew verification failures now preserve the
  original tool error and add actionable diagnostics.
- Docker verification checks daemon and Buildx availability before GHCR
  manifest and platform checks.
- Homebrew verification distinguishes remote formula state, local tap state,
  stable version mismatches, and fetch/checksum failures.
- `scripts/verify-release.sh` runs the installed quickstart smoke after public
  install verification, and `--json` records the installed quickstart gate plus
  the checked coverage list, including CLI `agent-route` and MCP `agent_route`.
- `scripts/post-release-verify.sh` wraps release verification, JSON summary
  persistence, and generated status-summary updates into one post-release
  command.
- CI covers the installer fallback, GitHub CLI auth failure, asset-download
  fallback, asset-unreachable diagnostic, Docker failure, Homebrew failure, and
  JSON summary paths through dedicated smoke scripts.
- `scripts/prepare-release.sh --dry-run v0.1.12` passed before the release and
  previewed the expected Cargo version, CHANGELOG section, and install example
  updates.

Known local environment caveats on the current development machine:

- GitHub CLI auth was refreshed on 2026-07-14 after the previous token returned
  `401 Unauthorized`.
- Docker CLI is installed, but the local Docker daemon is not running, so
  `docker pull` / `docker run` were not executed locally. GHCR manifest
  inspection confirmed the published multi-arch image.

## Next

- Keep the README/demo/benchmark path centered on the AI-agent first-read
  workflow: `index_project`, `project_overview`, `context_pack`, and
  `impact_analysis`.
- Tighten language resolver edge cases only when they materially improve
  context routing or impact triage.
- Keep benchmark evidence current when context-pack ranking or continuation
  behavior changes.
- Keep prompt templates, client examples, and adoption checks aligned with
  `selection_rank`, `selection_reason`, and `continuation_summary.next_action`
  when the first-read contract changes.
- Keep `scripts/installed-quickstart-smoke.sh` green after install, MCP, or
  first-read workflow changes.

For accuracy boundaries and current non-goals, see
[Known limitations](known-limitations.md).
