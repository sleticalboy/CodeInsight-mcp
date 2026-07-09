# Current Status

CodeInsight is an early MVP code-intelligence server. It is useful for local
repository indexing, navigation, and AI-agent context gathering, but it is not
yet a complete language-server-grade code-analysis platform.

## Implemented

- Rust CLI entrypoint.
- Local SQLite index cache under `.codeinsight/`.
- Incremental indexing with file-hash skips and stale file cleanup.
- Index metadata with schema and index version tracking.
- Per-file indexing errors in reports without aborting the whole project scan.
- Tree-sitter parsing for TypeScript/JavaScript, Python, Go, Rust, Java, C,
  C++, C#, PHP, and Ruby.
- Symbol extraction for common declarations.
- Repository overview, dependency graph, text reference search, impact
  analysis, context packs, and call graph tools with imported target hints.
- Relative file resolution for local dependency graph edges.
- Embedding provider interface, provider status reporting, and local semantic
  search paths over local vectors.
- Local semantic chunk index storage with optional deterministic local-hash
  embedding generation.
- CLI commands: `index`, `init-config`, `config-status`, `overview`,
  `symbols`, `outline`, `dependency-graph`, `impact-analysis`,
  `find-references`, `semantic-search`, `semantic-index`, `embedding-status`,
  `context-pack`, `callers`, and `callees`.
- MCP stdio `initialize`, `tools/list`, and `tools/call`.
- MCP tool argument validation with stable JSON-RPC errors.
- Fixture-based CLI and MCP stdio integration tests.
- Local smoke scripts for MCP stdio, semantic search, Docker, release install,
  and benchmark fixtures.

## Next

- Model JavaScript package condition and multi-wildcard edge cases beyond package discovery.
- Keep tightening JavaScript package metadata edge cases where they improve local code navigation.
- Continue tightening README/docs routing so the README stays entry-level.

For accuracy boundaries and current non-goals, see
[Known limitations](known-limitations.md).
