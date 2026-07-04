# CodeInsight MCP Server MVP Backlog

## MVP Objective

Build a local MCP server that helps AI agents understand a repository through indexed symbols, file outlines, dependency relationships, references, and compact context packs.

## Scope

Initial supported languages:

- TypeScript / JavaScript
- Python
- Go
- Rust

Rust is included early so the project can index itself during development.

## P0

### Repository Foundation

- [x] Initialize Rust CLI project.
- [x] Add local SQLite index store.
- [x] Add Tree-sitter parser integration.
- [x] Add basic symbol extraction tests.
- [x] Add project planning docs.
- [ ] Add release build workflow.
- [ ] Add fixture-based integration tests.

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
- [ ] JSON schema validation for tool args.

### Indexing

- [x] Skip noisy directories.
- [x] Store files and symbols.
- [x] Track file hash.
- [ ] Skip unchanged files during incremental indexing.
- [ ] Store index metadata and schema version.
- [ ] Report parse errors without failing whole index.

### Agent Context

- [ ] Define `context_pack` output schema.
- [ ] Implement token budget estimation.
- [ ] Rank files by seed symbol matches.
- [ ] Return line ranges with reasons.
- [ ] Add `context_pack` CLI command.
- [ ] Add `context_pack` MCP tool.

## P1

### References

- [x] Extract import/require/use/package dependencies.
- [ ] Store file-to-file dependencies.
- [x] Implement `dependency_graph`.
- [ ] Implement simple textual reference search scoped by indexed files.
- [ ] Add `find_references` tool.

### Call Graph

- [ ] Extract function call expressions.
- [ ] Resolve direct same-file calls.
- [ ] Resolve imported calls where obvious.
- [ ] Implement `callers`.
- [ ] Implement `callees`.
- [ ] Include confidence score.

### Quality

- [ ] Add sample fixture repositories.
- [ ] Add CLI integration tests with `assert_cmd`.
- [ ] Benchmark 10k, 50k, and 100k line repositories.
- [ ] Document known accuracy limits.

## P2

### Language Expansion

- [ ] Java
- [ ] C / C++
- [ ] C#
- [ ] PHP or Ruby

### Semantic Search

- [ ] Provider interface for embeddings.
- [ ] Optional local embedding index.
- [ ] Hybrid symbol + semantic ranking.

### Distribution

- [ ] GitHub release workflow.
- [ ] Homebrew formula.
- [ ] Docker image.
- [ ] MCP client configuration examples.

## Near-Term Milestones

### v0.1.0

- CLI index/search/outline stable enough for local use.
- MCP server can execute P0 tools through `tools/call`.
- Project can index itself and at least three external repositories.

### v0.2.0

- Dependency graph and references available.
- `context_pack` useful for bugfix and code-reading tasks.
- Incremental indexing avoids full rebuild on unchanged files.

### v0.3.0

- Basic call graph available.
- More robust TypeScript, Python, Go, and Rust extraction.
- Benchmark report for token/context reduction.
