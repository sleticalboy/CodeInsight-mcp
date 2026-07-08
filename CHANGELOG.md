# Changelog

All notable changes to CodeInsight MCP Server will be documented in this file.

The format is based on Keep a Changelog, and this project follows semantic versioning once tagged releases begin.

## [Unreleased]

### Changed

- `find_references` now filters obvious comment-only and string-only matches before ranking results.
- `find_references` now downranks test and fixture files so production references are less likely to be hidden by low-value matches.
- `context_pack` now applies the same low-value file penalty to inferred symbol, reference, dependency, and call-graph ranges from test and fixture files.
- `context_pack` now promotes test and fixture ranges when the task asks for tests, specs, coverage, regression, or when an explicit seed file is test-like.
- `context_pack` file-level reasons now identify the dominant selected source, such as `seed_file`, `symbol_definition`, `reference`, `call_graph`, `semantic`, or `dependency`.

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
