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
- Dependency graph with local resolution, source/target file filters, language
  filters, summaries, top source stats, and top target stats.
- Text reference search, impact analysis, token-budgeted context packs, reading
  plans, and call graph tools with imported target hints.
- Local dependency resolution for common import/include/use forms across the
  supported languages, including JavaScript/TypeScript package metadata,
  workspaces, Python relative imports, Rust modules, Go modules, Java/C#/PHP
  namespace imports, Ruby `require_relative`, and C/C++ local includes.
- Imported `callee_file` hints for obvious local calls in JavaScript/TypeScript,
  Python, Rust, Go, Java, C#, PHP, and Ruby.
- Embedding provider interface, provider status reporting, and local semantic
  search paths over local vectors.
- Local semantic chunk index storage with optional deterministic local-hash
  embedding generation.
- `context_pack` semantic status, reading plan, and file-scoped follow-up tool
  suggestions for `file_outline`, `impact_analysis`, `dependency_graph`, and
  focused `context_pack` calls.
- `context_pack` budget metadata, bounded omitted-candidate follow-ups, and a
  `continuation_summary` that lets MCP clients expose a single next action
  after the initial reading plan.
- CLI commands: `index`, `init-config`, `config-status`, `overview`,
  `symbols`, `outline`, `dependency-graph`, `impact-analysis`,
  `find-references`, `semantic-search`, `semantic-index`, `embedding-status`,
  `context-pack`, `callers`, and `callees`.
- MCP stdio `initialize`, `tools/list`, and `tools/call`.
- MCP tool argument validation with stable JSON-RPC errors.
- Fixture-based CLI and MCP stdio integration tests.
- Local smoke scripts for MCP stdio, semantic search, Docker, release install,
  and benchmark fixtures.
- Release, Docker image, Homebrew tap sync, install, verify, and release-note
  helper scripts.
- Published and verified `v0.1.10` with GitHub Release assets, Docker
  multi-arch images, public install script, and Homebrew tap formula.

## Next

- Add broader real-repository benchmark evidence for token/context reduction.
- Keep tightening JavaScript package-manager and bundler-specific edge cases
  where they improve local code navigation.
- Continue tightening README/docs routing so the README stays entry-level while
  detailed operational guidance stays in `docs/`.

For accuracy boundaries and current non-goals, see
[Known limitations](known-limitations.md).
