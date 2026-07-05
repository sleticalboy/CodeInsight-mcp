# Changelog

All notable changes to CodeInsight MCP Server will be documented in this file.

The format is based on Keep a Changelog, and this project follows semantic versioning once tagged releases begin.

## [Unreleased]

### Added

- Release install smoke script and CI coverage for packaged installer artifacts.
- `context_pack` now accepts file seeds through CLI `--file` and MCP `files`.
- `context_pack` now applies explicit candidate scoring before token-budget selection.
- File-seeded `context_pack` output now selects header/import context and primary top-level symbols instead of fixed first-file chunks.
- `context_pack` now caps large symbol and merged ranges so small token budgets retain useful file context.
- `context_pack` now uses task keywords as a lightweight relevance boost for symbols, references, and local dependencies.

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
