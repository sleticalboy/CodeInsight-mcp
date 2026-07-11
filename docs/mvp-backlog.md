# CodeInsight MCP Server MVP Backlog

## MVP Objective

Build a local MCP server that helps AI agents understand a repository through indexed symbols, file outlines, dependency relationships, references, and compact context packs.

## Scope

Current MVP supported languages:

- TypeScript / JavaScript
- Python
- Go
- Rust
- Java
- C / C++
- C#
- PHP
- Ruby

Rust is included early so the project can index itself during development.

## P0

### Repository Foundation

- [x] Initialize Rust CLI project.
- [x] Add local SQLite index store.
- [x] Add Tree-sitter parser integration.
- [x] Add basic symbol extraction tests.
- [x] Add project planning docs.
- [x] Add release build workflow.
- [x] Add fixture-based integration tests.

### CLI

- [x] `codeinsight index <root>`
- [x] `codeinsight overview <root>`
- [x] `codeinsight symbols <root> <query>`
- [x] `codeinsight outline <path>`
- [x] `codeinsight serve --transport stdio`

### MCP

- [x] `initialize`
- [x] `tools/list`
- [x] `tools/call` for `index_project`
- [x] `tools/call` for `project_overview`
- [x] `tools/call` for `symbol_search`
- [x] `tools/call` for `file_outline`
- [x] JSON schema validation for tool args.
- [x] End-to-end stdio smoke script.

### Indexing

- [x] Skip noisy directories.
- [x] Store files and symbols.
- [x] Track file hash.
- [x] Skip unchanged files during incremental indexing.
- [x] Store index metadata and schema version.
- [x] Remove stale index entries for deleted files.
- [x] Report parse errors without failing whole index.

### Agent Context

- [x] Define `context_pack` output schema.
- [x] Implement token budget estimation.
- [x] Rank files by seed symbol matches.
- [x] Support file seeds in `context_pack`.
- [x] Return line ranges with reasons.
- [x] Add `context_pack` CLI command.
- [x] Add `context_pack` MCP tool.
- [x] Add auto-entrypoint seed selection for `context_pack`.
- [x] Add `reading_plan` with next actions, questions, and MCP-ready suggested tools.
- [x] Add semantic status reporting and local semantic chunk fallback signals.
- [x] Add budget metadata for requested/applied token budgets and selected vs
  omitted candidate counts.
- [x] Add bounded omitted-candidate follow-ups for files excluded from the
  selected context.
- [x] Add `continuation_summary` so clients can expose a single next action
  without interpreting budget counters directly.

## P1

### References

- [x] Extract import/require/use/package dependencies.
- [x] Store file-to-file dependencies.
- [x] Implement `dependency_graph`.
- [x] Add dependency graph filters by touching file and source language.
- [x] Add dependency graph summaries, top sources, and top targets.
- [x] Implement simple textual reference search scoped by indexed files.
- [x] Add `find_references` tool.

### Call Graph

- [x] Extract function call expressions.
- [x] Resolve direct same-file calls.
- [x] Resolve imported calls where obvious.
- [x] Add imported `callee_file` hints for obvious local calls across
  JavaScript/TypeScript, Python, Rust, Go, Java, C#, PHP, and Ruby.
- [x] Implement `callers`.
- [x] Implement `callees`.
- [x] Include confidence score.

### Agent Recommendations

- [x] Add `project_overview.recommended_next_tools`.
- [x] Scope overview dependency graph recommendations to source entrypoints when available.
- [x] Scope dependency reading-plan suggestions to the selected context file.
- [x] Document recommendation priority bands and suggested-tool contracts.

### Quality

- [x] Add sample fixture repositories.
- [x] Add CLI integration tests with `assert_cmd`.
- [x] Add real-repository smoke benchmark.
- [x] Benchmark representative 10k+, 50k+, and 100k+ line repositories.
- [x] Document known accuracy limits.

## P2

### Language Expansion

- [x] Java
- [x] C / C++
- [x] C#
- [x] PHP
- [x] Ruby

### Semantic Search

- [x] Provider interface for embeddings.
- [x] Optional local embedding index.
- [x] Hybrid symbol + semantic ranking skeleton.
- [x] Provider status reporting.
- [x] Incremental semantic chunk storage with unchanged-vector preservation.
- [x] Local deterministic `local-hash` provider for smoke and preview flows.

### Distribution

- [x] GitHub release workflow.
- [x] Release install script.
- [x] Homebrew formula.
- [x] Docker image.
- [x] MCP client configuration examples.

## Near-Term Milestones

### v0.1.0

- CLI index/search/outline stable enough for local use.
- MCP server can execute P0 tools through `tools/call`.
- Project can index itself and at least three external repositories.

### v0.2.0

- Dependency graph and references available.
- `context_pack` useful for bugfix and code-reading tasks.
- `context_pack` exposes budget, continuation, and omitted-candidate metadata
  for multi-step code-reading workflows.
- Incremental indexing avoids full rebuild on unchanged files.

### v0.3.0

- Basic call graph available.
- More robust TypeScript, Python, Go, Rust, Java, C/C++, C#, PHP, and Ruby extraction.
- Benchmark report for token/context reduction.

### Release Readiness

- [x] Release build workflow.
- [x] Docker image workflow.
- [x] Release install smoke script.
- [x] Release note extraction and prepare-release scripts.
- [x] Homebrew formula update/sync scripts.
- [x] Release verification script.
- [x] Run a release-readiness rehearsal covering format, tests, MCP smoke,
  semantic smoke, and release install smoke.
- [x] Decide the next public tag/version and prepare release notes.
- [x] Verify GitHub Release, Docker image, and Homebrew tap after tagging.
- [x] Shorten generated release notes for large accumulated changelog sections.
- [ ] Run a clean release-readiness rehearsal from a fresh checkout before the
  next public tag.
