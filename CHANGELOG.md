# Changelog

All notable changes to CodeInsight MCP Server will be documented in this file.

The format is based on Keep a Changelog, and this project follows semantic versioning once tagged releases begin.

## [Unreleased]

### Changed

- `scripts/verify-release.sh` now reports a specific local network diagnostic
  when GitHub Release assets exist in API metadata but
  `github.com/releases/download` is unreachable, and supports an explicit
  metadata-only override for that case.
- Install docs now show pipe-compatible commands for version-pinned,
  custom-directory, and authenticated remote installer use.
- Added `scripts/installed-quickstart-smoke.sh` to verify an installed binary
  can complete the quickstart CLI and MCP stdio flow against a temporary
  project outside the source checkout, and documented it as a release
  readiness/runbook gate.
- `scripts/verify-release.sh` now runs the installed quickstart smoke after
  public install verification and records the gate in `--json` output.
- `scripts/install.sh` now applies bounded timeouts to GitHub CLI and curl
  release downloads before falling back or failing.
- Added `scripts/update-release-status.sh` to refresh the generated
  `docs/status.md` release verification summary from
  `scripts/verify-release.sh --json` output.
- Added `scripts/post-release-verify.sh` to run release verification, save the
  JSON summary, and refresh `docs/status.md` through one post-release command.
- Added `docs/release-commands.md` as a short release-maintenance command
  index for prepare, publish, verify, and status-update operations.
- Added `docs/maintainer-checklist.md` to collect routine development, PR,
  release, and support checks in one place.
- Added `scripts/docs-link-smoke.sh` to catch broken local Markdown links in
  README, CHANGELOG, and docs pages.
- Added `scripts/docs-positioning-smoke.sh` to keep entry docs linked to the
  local-first AI-agent workflow, support checklist, and limitations docs.
- Added `scripts/docs-benchmark-smoke.sh` to keep README demo/benchmark claims,
  benchmark reports, and benchmark fixture profiles aligned.
- Added `scripts/docs-smoke.sh` as the single local entrypoint for all docs
  smoke checks.
- Added `scripts/release-tooling-smoke.sh` as the single local entrypoint for
  release-tooling smoke checks.
- Added `scripts/local-ci-smoke.sh` as the single local entrypoint for the
  standard non-network CI gate.
- Added `scripts/script-syntax-smoke.sh` as the single local entrypoint for
  shell script syntax checks.
- Added `scripts/verify-release-help-smoke.sh` to keep release verification
  help text contract checks out of CI workflow YAML.
- Added `scripts/prepare-release-smoke.sh` to keep release prep fixture checks
  out of CI workflow YAML.
- Added `scripts/update-homebrew-formula-smoke.sh` to keep Homebrew formula
  fixture checks out of CI workflow YAML.
- Added `scripts/release-notes-smoke.sh` to keep release note extraction
  contract checks out of CI workflow YAML.
- Renamed the CI release-notes smoke job to maintenance smoke to match its
  release notes, release tooling, script syntax, and docs coverage.
- Added `docs/maintenance-commands.md` as the short command index for local
  development and maintenance smoke checks.
- `scripts/local-ci-smoke.sh` now prints numbered stage labels before each
  gate.
- `scripts/release-tooling-smoke.sh` now prints numbered stage labels before
  each release-tooling smoke.
- `scripts/docs-smoke.sh` now prints numbered stage labels before each docs
  smoke.
- Smoke wrappers now share a common numbered-stage helper.
- `scripts/agent-router-demo.sh` now prints `context_pack` reading-plan step
  count and first next action.
- `scripts/agent-router-demo.sh` now fails fast when `context_pack` does not
  return selected files, reading-plan steps, or a first next action.
- `scripts/local-ci-smoke.sh` now includes the agent-router demo so local
  non-network checks cover the first-read product flow.
- CI now runs `scripts/agent-router-demo.sh` as its own job so the first-read
  product flow is covered remotely.
- `scripts/benchmark-smoke.sh` now enforces context-pack guardrails for first
  recommended tool, selected context, reading plan, token budget, and line
  reduction.
- Benchmark context-pack guardrails now support per-repository thresholds for
  selected files, ranges, reading-plan steps, token usage, and line reduction.
- `scripts/benchmark-smoke.sh` now supports `CODEINSIGHT_BENCH_REUSE_REPOS=1`
  and bounded low-speed Git clones for more reliable benchmark refreshes.
- `scripts/benchmark-smoke.sh` now supports `CODEINSIGHT_BENCH_REPOS` for
  running selected benchmark fixtures without overwriting the full reports by
  default.
- Docs benchmark smoke now checks benchmark report guardrail expectations
  against `scripts/benchmark-smoke.sh` exported profile configuration.
- CI now runs a lightweight `p-limit` benchmark subset smoke without
  overwriting the full benchmark reports and uploads the subset report as a
  workflow artifact.
- Added `scripts/benchmark-report-smoke.sh` so generated benchmark reports and
  CI subset artifacts fail fast when required summary, detail, or guardrail
  evidence is missing.
- Benchmark reports now include a `Key Results` section that summarizes
  context-pack routing, aggregate source-line compression, token usage,
  indexing time, guardrail failures, and truncation status.
- CI benchmark artifact upload now uses `actions/upload-artifact@v7` to avoid
  Node.js 20 runtime deprecation warnings.
- Added `scripts/workflow-actions-smoke.sh` to keep checkout, artifact, Rust,
  and Docker workflow actions on the expected major versions.
- CI benchmark subset runs now publish the report `Key Results` and summary
  table into the GitHub Actions run summary alongside the full artifact.
- Benchmark run summaries now include direct workflow run and artifact links
  when the CI environment provides them.
- Release readiness and maintenance docs now explain how to inspect the CI
  benchmark summary and download the subset artifact for full guardrail detail.
- Added `scripts/benchmark-artifact-smoke.sh` to download a CI benchmark
  artifact by run id and validate the contained report.
- Release command and runbook docs now include the benchmark artifact smoke as
  a pre-tagging gate after release-prep CI completes.

## [0.1.12] - 2026-07-14

### Changed

- `scripts/install.sh` now falls back to `curl` when `gh release download`
  fails, so invalid GitHub CLI auth does not block public release installs.
- `scripts/verify-release.sh` now adds actionable diagnostics when GitHub CLI
  auth failures or API rate limits block release verification.
- `scripts/verify-release.sh` now checks direct HTTP downloadability for every
  expected GitHub Release archive.
- `scripts/verify-release.sh` now gives actionable Docker diagnostics for
  daemon, Buildx, registry, and platform verification failures.
- `scripts/verify-release.sh` now gives actionable Homebrew diagnostics for
  tap state, formula version, fetch, checksum, and local environment failures.
- `scripts/verify-release.sh` now routes GitHub, Docker, and Homebrew command
  checks through a shared status-capture helper to reduce release-script drift.
- `scripts/verify-release.sh --json` now prints a final machine-readable
  verification summary after all release gates pass.

## [0.1.11] - 2026-07-14

### Changed

- `scripts/prepare-release.sh --dry-run` now uses `git diff --no-index` so
  release previews still work when another toolchain shadows the system
  `diff`.
- Smoke and release-install scripts now locate release binaries through Cargo
  metadata so they work when `CARGO_TARGET_DIR` points outside the repository.
- `scripts/verify-release.sh` now reports a pending Homebrew tap PR when the
  shared tap formula has not been merged to `main` yet.
- `scripts/extract-release-notes.sh` now supports compact summary output for
  GitHub Release pages while keeping full changelog extraction as the default.
- `dependency_graph` CLI and MCP outputs now support `offset` pagination and
  return `page_size` / `has_more` metadata for large dependency graphs.
- `context_pack` now returns structured `budget` metadata with requested and
  applied token budgets, selected/omitted candidate counts, and truncation reason.
- `context_pack` now returns bounded `omitted_candidates` follow-ups for
  high-ranked files that were excluded from the selected context.
- `context_pack` now returns `continuation_summary` so clients can expose a
  single next action without interpreting budget counters directly.
- MCP stdio smoke coverage now validates `context_pack` budget metadata and
  omitted-candidate follow-up calls.
- Added a client workflow guide that connects overview, context packs, reading
  plans, continuation follow-ups, and impact analysis.
- Added an adoption checklist for validating CodeInsight as an AI-agent code
  context router in a real repository.
- Added a two-minute demo script for presenting the `index_project`,
  `project_overview`, `context_pack`, and `impact_analysis` flow.
- Added copy-paste agent prompt templates for first reads, change preflight,
  budget continuation, and review planning.
- Added a public MVP release-readiness checklist and recorded a fresh-checkout
  rehearsal for the next public tag.
- Benchmark reports now include context-pack applied budget, omitted-file, and
  continuation status metrics, and the smoke/large reports were refreshed.
- Added CLI coverage and limitation docs for dependency package `exports`
  mappings that point at extensionless local files.
- Fixed package `exports` and `imports` subpath matching so exact entries and
  more specific wildcard patterns take precedence over broader wildcard entries.
- Fixed TypeScript/JavaScript `paths` alias matching to use the same exact and
  more-specific wildcard precedence when resolving imports.
- Added CLI coverage for local `package.json#imports` exact and more-specific
  wildcard precedence in dependency and callee resolution.
- Added CLI coverage for workspace protocol package `exports` exact and
  more-specific wildcard precedence.
- Added CLI coverage for dependency package root `exports` array fallbacks that
  skip disabled, external, and missing targets before resolving a local file.
- Added CLI coverage for dependency package root `exports` entries remapped by
  a package-level `browser` string target.
- Go same-module import resolution now prefers ordinary implementation files
  over package `doc.go` and `_test.go` files when choosing a package representative.
- Added CLI coverage for explicit Go import aliases flowing through
  dependency metadata and package-qualified `callee_file` hints.
- Added CLI coverage for Python relative package imports resolving through
  package `__init__.py` files and providing member-call `callee_file` hints.
- Added CLI coverage for Python absolute package imports resolving through
  package `__init__.py` files with explicit aliases.
- Added CLI coverage for Rust `self::` use imports from a `mod.rs` module to a
  nested sibling module file with `callee_file` hints.
- Added CLI coverage for Rust `mod` declarations preferring sibling `foo.rs`
  before `foo/mod.rs`.
- Updated MVP status docs after the verified `v0.1.10` release.

## [0.1.10] - 2026-07-11

### Changed

- `dependency_graph` CLI and MCP output can now be filtered by source/target file and indexed language, and includes summary, top source, and top target stats to reduce large-repository dependency noise.
- `project_overview.recommended_next_tools` now scopes `dependency_graph` suggestions to the detected source entrypoint file when available.
- `context_pack.reading_plan[].suggested_tool` now scopes dependency follow-up recommendations to the selected context file.
- Python `callees` output now preserves member call names such as `audit.record` and can attach `callee_file` hints when a resolved local `from ... import ...` dependency contains the called member symbol.
- Python absolute `from ... import name as alias` imports are now covered for direct alias call `callee_file` hints such as `shared_ping`, including package `__init__.py` fallback when the member is defined in the package file.
- Python parenthesized multi-line `from ... import (...)` declarations and imported submodule aliases are now covered for `callee_file` hints such as `shared_tools.pong`.
- Python dependency graph resolution now expands `from ... import ...` member targets and resolves relative `from .` / `from ..` imports to nearby package `.py` files.
- Rust `callees` output now preserves scoped call names such as `audit.record` and can attach `callee_file` hints when local `use crate::`, `use self::`, or `use super::` dependencies resolve to a file containing the called symbol.
- Rust dependency graph resolution now resolves obvious local `crate::`, `self::`, and `super::` use paths to module files while leaving external crate imports unresolved.
- Go `callees` output now preserves package-qualified call names such as `auth.Login` and can attach `callee_file` hints when same-module imports resolve to indexed local package files.
- Java `callees` output now preserves class-qualified call names such as `AuthService.login` and can attach `callee_file` hints for resolved local class and static imports.
- Java wildcard imports such as `import com.example.reporting.*` can now provide `callee_file` hints for explicit class-qualified calls like `Report.log` without marking the wildcard dependency itself as resolved.
- Java same-package class-qualified calls such as `LocalFormatter.decorate` can now receive `callee_file` hints from the source file's `package` declaration without marking the package dependency as resolved.
- C# `callees` output now preserves qualified call names such as `Audit.Record` and can attach `callee_file` hints for resolved local alias and static `using` directives.
- C# `this.`-qualified instance calls such as `this.users.Find` and `this.LocalTag` are now normalized to the field or method name, enabling the same type-binding hints and same-file call graph behavior as unqualified calls.
- C# same-namespace class-qualified calls such as `LocalFormatter.Normalize` can now receive `callee_file` hints from the source file's `namespace` declaration without marking the namespace dependency as resolved.
- C# simple type bindings such as `UserService users`, `UserService targetUsers = new()`, `App.Services.UserService backupUsers`, alias-based `Repo repoUsers`, or `var createdUsers = new UserService()` can now provide `callee_file` hints for instance calls like `users.Find` when the type is explicit, constructed locally, or expanded through a local `using` alias.
- C# dependency graph resolution now resolves obvious same-repository `using` directives under common roots such as `src`, including alias/static using targets and namespace directories with representative `.cs` files.
- PHP `callees` output now preserves scoped call names such as `AuditLog.record` and can attach `callee_file` hints for resolved local class and function `use` imports.
- PHP grouped `use` declarations such as `use App\Support\{Metrics as MetricsAlias}` and grouped function imports now resolve to local files and can provide `callee_file` hints.
- Ruby `callees` output now preserves member call names such as `Audit.record` and can attach `callee_file` hints for resolved local `require_relative` imports.
- Ruby dependency graph resolution now resolves `require_relative` imports to neighboring local `.rb` files while leaving gem-style `require` imports unresolved.
- C/C++ local includes can now provide `callee_file` hints for calls to indexed inline/header function definitions such as `shared_value`.
- C/C++ symbol extraction now indexes simple function prototype declarations, enabling local include `callee_file` hints for header-declared calls such as `declared_value`.
- C/C++ dependency graph resolution now resolves quoted local includes from the source file directory, explicit relative paths, and obvious repository-root subpaths while leaving system includes unresolved.
- PHP dependency graph resolution now resolves obvious same-repository namespace `use` imports under common roots such as `src`, including `App\` PSR-4 style paths and function imports when a matching file exists.
- Java dependency graph resolution now resolves obvious same-repository class imports under common source roots such as `src/main/java`, including static member imports that can fall back to the containing class file.
- Go dependency graph resolution now resolves same-module package imports declared by the nearest `go.mod` to representative local `.go` files.
- JavaScript and TypeScript package resolution now follows same-repository `package.json` and `pnpm-workspace.yaml` workspaces for local package `exports` targets.
- JavaScript and TypeScript package resolution now follows relative `workspace:` dependency paths such as `workspace:../pkg` for local package `exports` targets.
- JavaScript and TypeScript package resolution now treats workspace version protocols such as `workspace:*`, `workspace:^`, `workspace:~`, and `workspace:<semver>` as local workspace package references.
- JavaScript and TypeScript package resolution now treats `catalog:` and `catalog:name` dependency versions as external catalog references instead of local workspace package references.
- JavaScript and TypeScript package resolution now parses `pnpm-workspace.yaml` `catalog` and `catalogs` metadata when classifying catalog dependency sources.
- JavaScript and TypeScript package `exports` and `imports` resolution now tries array fallback targets in order.
- JavaScript and TypeScript package `exports` and `imports` array fallback resolution now skips `null` entries while continuing to later targets.
- JavaScript and TypeScript package `exports` and `imports` array fallback resolution now skips external package targets while continuing to later local targets.
- JavaScript and TypeScript package `exports` and `imports` matched conditional external package targets now block local fallback resolution instead of falling through to later conditions.
- JavaScript and TypeScript package `exports` resolution now treats explicit `null` subpath mappings as disabled and does not fall back to package-relative files.
- JavaScript and TypeScript dependency package `browser` object remaps are now covered for disabled `false` entries in CLI dependency graph resolution.
- JavaScript and TypeScript dependency package `browser` object remap keys are now covered for both `./path` and `path` forms.
- JavaScript and TypeScript dependency package `browser` object remap values now skip external package targets instead of treating them as package-local files.
- JavaScript and TypeScript dependency package root `browser` string entries now skip external package targets instead of treating them as package-local files.
- JavaScript and TypeScript dependency package `browser` object remap values are now covered for absolute path and non-string entries that should block local resolution.
- JavaScript and TypeScript package metadata root fallbacks through `module`, `main`, `types`, and `typings` now skip external, absolute, or non-string targets.
- JavaScript and TypeScript package subpath metadata fallback is now covered for package-relative files, directory indexes, missing files, and disabled `exports` entries.
- JavaScript and TypeScript package `exports` resolution now treats matched conditional `null` branches as disabled instead of continuing to later conditions.
- JavaScript and TypeScript package `imports` resolution now treats matched conditional `null` branches as disabled and does not fall through to `tsconfig` path aliases.
- JavaScript and TypeScript package `exports` and `imports` condition priority can now be configured with `[javascript].package_conditions`.
- JavaScript and TypeScript path alias, package `exports`, and package `imports` resolution now supports multiple `*` captures in one mapping.
- JavaScript and TypeScript package resolution now applies common `package.json#browser` string and object remaps when resolving package entries.
- JavaScript and TypeScript workspace package resolution is now covered for pnpm and Yarn workspace version protocol variants, including `workspace:~` and `workspace:<semver>`.
- JavaScript and TypeScript workspace package resolution is now covered for negated `package.json#workspaces.packages` patterns such as `!packages/legacy-*`.
- JavaScript and TypeScript workspace package resolution is now covered for negated array-form `package.json#workspaces` patterns such as `["packages/*", "!packages/legacy-*"]`.
- JavaScript and TypeScript workspace package resolution is now covered for recursive array-form `package.json#workspaces` patterns such as `["packages/**", "!packages/legacy/**"]`.
- JavaScript and TypeScript workspace package resolution is now covered for recursive Yarn-style `package.json#workspaces.packages` patterns such as `["packages/**", "!packages/legacy/**"]`.
- JavaScript and TypeScript workspace discovery now supports recursive `**` workspace package patterns such as `packages/**`.
- JavaScript and TypeScript workspace discovery now honors negated workspace package patterns such as `!packages/legacy/**`.
- JavaScript and TypeScript package resolution now follows local `package.json#imports` aliases such as `#internal/*`.
- TypeScript and JavaScript config resolution now follows relative `extends` chains for inherited `baseUrl` and `paths` aliases.
- Added a documentation index and shortened the README document-link list.
- Added a current-status document and shortened the README status section.
- Moved detailed MCP tool summaries from README into the MCP tools guide.
- Added an MCP tools guide and shortened the README MCP section to the recommended first-read flow.
- Added a CLI usage guide and shortened the README command examples to the core index/overview/context/MCP loop.
- Moved detailed install, Homebrew, and Docker usage notes from README into a dedicated install document.
- Updated release-prep automation and CI smoke coverage to maintain the install-document version example.
- Added a navigation-tools contract document and shortened the README references/call-graph sections.
- Shortened the README impact-analysis section to point at the detailed impact-analysis contract.
- Moved detailed semantic search/index/status workflow notes from README into the embedding providers documentation.
- Added a first-read workflow document and shortened the README overview/context-pack sections to point at the detailed contract.
- Added a recommendation contract document for `recommended_next_tools` and `reading_plan[].suggested_tool` fields, priorities, and client sorting guidance.
- `context_pack.reading_plan[].suggested_tool` now includes `priority` so clients can sort follow-up calls consistently with overview recommendations.
- `overview` / `project_overview` recommended tools now include `priority` so clients can display next calls without relying on array order.
- `scripts/mcp-stdio-smoke.sh` now executes selected `project_overview.recommended_next_tools` calls so overview recommendations are verified as usable MCP arguments.
- `scripts/mcp-stdio-smoke.sh` now executes the first explicit and auto `context_pack.reading_plan[].suggested_tool` calls to verify suggested MCP arguments are usable.
- `context_pack.reading_plan` entries now include `suggested_tool` objects that map reading steps to MCP-ready follow-up calls.
- `context_pack.reading_plan` entries now include stable `next_action` hints and guiding `question` text for client follow-up routing.
- `scripts/mcp-stdio-smoke.sh` now asserts `project_overview.recommended_next_tools` and `context_pack.reading_plan` so MCP client-facing response fields are covered by release smoke tests.
- `context_pack` now returns a structured `reading_plan` with ordered files, focus text, reasons, scores, and line ranges derived from the final selected context.
- `overview` / `project_overview` now returns `recommended_next_tools` with MCP-ready next-call suggestions and argument shapes.
- `scripts/mcp-stdio-smoke.sh` now verifies the recommended MCP first-read chain: `index_project`, `project_overview`, and auto-entrypoint `context_pack`.
- `context_pack` now returns structured `seed_strategy` and `selected_seeds` fields so clients can inspect explicit, auto-entrypoint, and source-fallback seed decisions without parsing summary text.
- `context_pack` now auto-selects a source entrypoint from `project_overview` when no seed symbols or files are provided, with source-file fallback for repositories without obvious entrypoints.
- `overview` / `project_overview` now returns an agent-ready repository briefing with total lines, symbol-kind counts, richer directory stats, dependency/call summaries, entrypoint candidates, and index metadata.
- `overview` / `project_overview` now annotates main directories and entrypoint candidates with role hints, and entrypoints with normalized confidence scores.
- Release builds now use Node 24-compatible artifact upload/download actions and cap Linux cross-toolchain installation at 10 minutes.

## [0.1.9] - 2026-07-08

### Added

- `scripts/prepare-release.sh` now automates release prep for Cargo version metadata, README install examples, changelog sections, and Cargo.lock refresh.
- `scripts/prepare-release.sh` now rejects duplicate or lower release versions and gives clearer guidance for empty Unreleased changelog sections.
- `scripts/latest-changelog-version.sh` and `extract-release-notes.sh latest` now let CI validate the latest release notes without hard-coding a version.
- `scripts/verify-release.sh` now consolidates GitHub Release, install script, Docker manifest, and Homebrew tap verification for published tags.
- CI release notes smoke now checks for changelog headings instead of rejecting release note text that mentions `Unreleased`.

## [0.1.8] - 2026-07-08

### Changed

- `find_references` now filters obvious comment-only and string-only matches before ranking results.
- `find_references` now downranks test and fixture files so production references are less likely to be hidden by low-value matches.
- `context_pack` now applies the same low-value file penalty to inferred symbol, reference, dependency, and call-graph ranges from test and fixture files.
- `context_pack` now promotes test and fixture ranges when the task asks for tests, specs, coverage, regression, or when an explicit seed file is test-like.
- `context_pack` file-level reasons now identify the dominant selected source, such as `seed_file`, `symbol_definition`, `reference`, `call_graph`, `semantic`, or `dependency`.
- `context_pack` now returns structured `source` fields on files and ranges so clients can filter context without parsing reason text.
- `context_pack` now returns `score` fields on files and ranges so clients can sort, filter, or explain selected context.
- MCP client configuration docs now include a `context_pack` response example with structured `source` and `score` fields.

## [0.1.7] - 2026-07-08

### Changed

- Docker release images now publish multi-architecture manifests for `linux/amd64` and `linux/arm64`.
- Docker release builds now use native amd64/arm64 runner jobs before publishing the combined manifest.
- `scripts/docker-smoke.sh` now accepts `CODEINSIGHT_DOCKER_PLATFORM` for explicit platform smoke tests.

## [0.1.6] - 2026-07-08

### Added

- `version` CLI/MCP command and top-level `--version` flag for release verification and client diagnostics.
- `semantic-index` now preserves existing embeddings for unchanged chunks and only embeds chunks missing vectors for the selected provider/model.
- `semantic-index` now reports incremental chunk and embedding stats: `chunks_added`, `chunks_updated`, `chunks_removed`, `embeddings_generated`, and `embeddings_reused`.
- `semantic-index --explain` and MCP `semantic_index` `explain: true` now return per-chunk add/update/remove details for impact analysis workflows.
- `impact-analysis` CLI command and MCP `impact_analysis` tool for local single-hop impact reports from seed symbols or files.
- `impact-analysis` now accepts `--depth` / MCP `depth` and returns call/dependency `paths` for multi-hop impact explanations.
- `impact-analysis` now supports `--format summary|full` and `--evidence-limit` to keep large reports compact.
- `impact-analysis` now reports `risk_level`, `impact_counts`, and `top_reasons` for client-friendly summaries.
- Impact analysis scoring and risk-level rules are now documented and covered by stable fixture assertions.
- `impact-analysis` now returns `suggested_checks` with inferred validation commands and review checkpoints.
- `impact-analysis` now prefers configured suggested check commands from `.codeinsight/config.toml` before falling back to built-in command inference.
- `init-config` CLI command for creating a sample `.codeinsight/config.toml` project configuration.
- `init-config` now pre-fills test commands from common repository metadata when possible.
- `config-status` CLI command and MCP `config_status` tool for reporting project configuration visibility.
- `config-status` now reports malformed project config files through `parse_error` while `impact-analysis` fails with clear parse context.
- Config status documentation now includes missing, loaded, and malformed output examples for MCP clients.

## [0.1.5] - 2026-07-07

### Added

- OpenAI-compatible embedding HTTP transport for `/embeddings` with redacted API-key handling, response index ordering, and response-shape validation.
- `semantic-index` now batches embedding provider requests with `CODEINSIGHT_EMBEDDING_BATCH_SIZE`, defaulting to 64 chunks per request.
- `embedding-status` now reports the effective embedding batch size and its environment variable name.
- Optional OpenAI-compatible semantic smoke script that skips when `CODEINSIGHT_OPENAI_API_KEY` is not configured.
- `openai` embedding provider config skeleton with environment validation and redacted `embedding-status` reporting.

## [0.1.4] - 2026-07-07

### Added

- `context_pack` now returns `semantic_status` so clients can tell whether semantic vector or fallback ranges were available and selected.
- `context_pack` now adds semantic vector matches when the selected embedding provider/model has indexed vectors, while keeping the deterministic fallback path.
- `embedding-status` CLI command and `embedding_status` MCP tool for reporting provider config and local semantic vector status without network calls.
- Protocol-level mock HTTP tests for Ollama embedding requests, chunked responses, non-200 responses, and vector-count mismatches.

## [0.1.3] - 2026-07-07

### Added

- Preview `ollama` embedding provider using the local Ollama `/api/embed` HTTP endpoint.
- Optional Ollama semantic smoke script.
- Embedding provider contract documentation for current `local-hash` support and planned external-provider boundaries.
- Release runbook covering tagged releases, Homebrew tap sync, Docker publishing, and verification commands.

### Changed

- Semantic provider errors now point directly to `CODEINSIGHT_EMBEDDING_PROVIDER=local-hash` and list supported provider names for unknown providers.
- Release automation now publishes only the changelog section for the current tag.
- Release automation can update the shared Homebrew tap formula from packaged release assets when `HOMEBREW_TAP_TOKEN` is configured.
- Release automation now supports manual Homebrew tap sync for an existing tag through `workflow_dispatch` input `tag`.
- Homebrew install docs now point at the shared `sleticalboy/tap`.

## [0.1.2] - 2026-07-06

### Added

- JavaScript indexing now extracts CommonJS assignment symbols such as `module.exports`, `exports.foo`, and object method assignments.
- JavaScript indexing now extracts computed assignment method placeholders such as `app.<dynamic>` for dynamic method registration loops.
- JavaScript indexing now extracts function-valued variable declarations, named function expressions, and simple arrow-function assignments as callable symbols.
- JavaScript indexing now extracts object literal function properties such as `handlers.getUser`, `handlers.saveUser`, and nested object methods.
- JavaScript indexing now extracts destructured object and array binding symbols, including aliases, nested bindings, rest bindings, and function-valued default bindings.
- JavaScript call graph indexing now preserves member call targets such as `app.get`, resolves string computed calls such as `app["post"]`, and records variable computed calls as `app.<dynamic>`.
- JavaScript call graph indexing now preserves chained and optional member call targets such as `router.route.get`, `app.route.get`, and `app?.put`.
- JavaScript call graph indexing now attributes calls inside anonymous callbacks to contextual callers such as `it.<callback>` and `app.get.<callback>`.
- Benchmark profiles can assert static call target guardrails; the large Express fixture now checks `app.get`, `app.<dynamic>`, `app.route.get`, and `router.route.get` callers.
- Benchmark profiles can assert static call edge guardrails; the large Express fixture now checks callback caller attribution for `it.<callback> -> app.route.get` and `app.get.<callback> -> res.send`.
- Benchmark profiles can assert static symbol guardrails; the large Express fixture now checks function-value, object literal method, and destructured binding symbols such as `createError`, `User.index`, `METHODS`, and `Buffer`.
- Benchmark profiles now report context lines and line-reduction percentages for context packs.
- Benchmark profiles now report index budgets and fail when fixture index times exceed guardrail thresholds.
- Large repository benchmark profile and generated report for Express, Flask, Gin, and Tokio.
- `callers` and `callees` now include imported callee file hints when a call target matches a symbol in a resolved local dependency.
- Release install smoke script and CI coverage for packaged installer artifacts.
- `context_pack` now accepts file seeds through CLI `--file` and MCP `files`.
- `context_pack` now applies explicit candidate scoring before token-budget selection.
- File-seeded `context_pack` output now selects header/import context and primary top-level symbols instead of fixed first-file chunks.
- `context_pack` now caps large symbol and merged ranges so small token budgets retain useful file context.
- `context_pack` now uses task keywords as a lightweight relevance boost for symbols, references, and local dependencies.
- Checked-in polyglot fixture coverage for TypeScript, JavaScript, Python, Go, Rust, Java, C, C++, C#, PHP, and Ruby indexing.
- Homebrew formula for installing release assets.
- Docker image definition and smoke test script.
- GitHub Container Registry image publishing workflow for tagged releases.
- Java indexing for common classes, interfaces, enums, records, methods, constructors, fields, packages, imports, and method calls.
- C/C++ indexing for common functions, structs/classes, enums, typedefs, macro constants, includes, and calls.
- C# indexing for common classes, interfaces, structs, enums, records, methods, constructors, properties, fields, using directives, and calls.
- PHP indexing for common classes, interfaces, traits, enums, functions, methods, properties, constants, use declarations, and calls.
- Ruby indexing for common classes, modules, methods, singleton methods, constants, require directives, and calls.
- Embedding provider interface and preview `semantic_search` CLI/MCP contract with explicit unconfigured-provider errors.
- Local semantic chunk index storage and `semantic_index` CLI/MCP entry point for future embedding generation.
- Deterministic hybrid ranking skeleton that lets `context_pack` use local semantic chunks as supporting context.
- Optional local-hash embedding generation for semantic chunks when `CODEINSIGHT_EMBEDDING_PROVIDER=local-hash`.
- Local semantic vector search over generated embeddings with cosine ranking.
- Semantic smoke script and docs for the local `semantic-index` plus `semantic-search` loop.

### Changed

- Imported call target resolution now uses batched SQLite resolution instead of per-call queries.
- JSON output now serializes language names as `javascript` and `typescript` instead of enum-derived `java_script` and `type_script`.

## [0.1.1] - 2026-07-05

### Added

- Release asset installer for macOS and Linux.
- Smoke benchmark script for real public repositories.
- MCP stdio smoke script and client troubleshooting notes.

## [0.1.0] - 2026-07-05

Initial MVP release.

### Added

- Local-first Rust CLI and MCP stdio server.
- Tree-sitter based parsing for Python, JavaScript/TypeScript, Go, and Rust.
- Local SQLite index cache under `.codeinsight/`.
- Incremental indexing with file-hash skips.
- Stale index cleanup for deleted files.
- Index metadata with schema and index version tracking.
- Per-file indexing errors without aborting the full project scan.
- Symbol search.
- File outlines.
- Dependency graph with local relative-file resolution for supported import forms.
- Text-reference search.
- Deterministic `context_pack` output using symbols, references, and resolved local dependencies.
- Same-file static call graph tools: `callers` and `callees`.
- MCP `tools/call` support for MVP tools.
- MCP argument validation with stable JSON-RPC errors.
- Fixture-based CLI and MCP stdio integration tests.
- GitHub CI.
- Manual/tag-triggered release build workflow for Linux and macOS artifacts.
- Apache License 2.0.

### Known Limitations

- Reference search is text based, not LSP-grade semantic reference resolution.
- Dependency resolution is partial and local-first.
- Call graph support is same-file only.
- `context_pack` uses approximate token estimation and deterministic local heuristics.
