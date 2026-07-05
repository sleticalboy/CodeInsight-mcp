# CodeInsight MCP Server

CodeInsight MCP Server is a local-first code intelligence layer for AI coding agents.

The MVP focuses on a narrow, verifiable loop:

- index a local repository
- extract source symbols with Tree-sitter
- search symbols from a local SQLite index
- expose an MCP stdio server scaffold
- build toward agent-ready context packs

The product direction and execution plan live in:

- [Product prototype](docs/product-prototype.md)
- [Implementation plan](docs/implementation-plan.md)
- [MVP backlog](docs/mvp-backlog.md)
- [Known limitations](docs/known-limitations.md)

## Current Status

This repository is an early MVP scaffold. It is not yet a complete MCP code-analysis server.

Implemented:

- Rust CLI entrypoint
- local SQLite index cache under `.codeinsight/`
- incremental indexing with file-hash skips and stale file cleanup
- index metadata with schema and index version tracking
- per-file indexing errors in reports without aborting the whole project scan
- Tree-sitter parsing for TypeScript/JavaScript, Python, Go, and Rust
- symbol extraction for common declarations
- dependency graph, text reference search, context packs, and same-file call graph tools
- `index`, `overview`, `symbols`, `outline`, `dependency-graph`, `find-references`, `context-pack`, `callers`, and `callees` CLI commands
- MCP stdio `initialize`, `tools/list`, and `tools/call` for P0 tools
- MCP tool argument validation with stable JSON-RPC errors
- fixture-based CLI and MCP stdio integration tests

Next:

- file-to-file dependency resolution
- imported call resolution
- benchmark fixtures for larger repositories

## Install From Source

```bash
cargo install --path .
```

## CLI Usage

Index a repository:

```bash
cargo run -- index /path/to/repo --force
```

Print an overview:

```bash
cargo run -- overview /path/to/repo
```

Search symbols:

```bash
cargo run -- symbols /path/to/repo AuthService
```

Print a file outline:

```bash
cargo run -- outline /path/to/repo/src/auth.ts
```

Start the MCP stdio server scaffold:

```bash
cargo run -- serve --transport stdio
```

## MCP Tools

The stdio server currently exposes:

- `index_project`
- `project_overview`
- `symbol_search`
- `file_outline`
- `dependency_graph`
- `find_references`
- `context_pack`
- `callers`
- `callees`

Example `tools/call` request:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"symbol_search","arguments":{"root":"/path/to/repo","query":"AuthService","limit":5}}}
```

`find_references` is currently a fast text-reference pass over indexed files. It returns file, line, column, context, an approximate reference kind, and a confidence score. It is not yet a full language-server-grade semantic reference resolver.

`context_pack` combines symbol search and reference search into a token-budgeted context bundle for agents. The first version is deterministic and local-only; it does not use embeddings.

`callers` and `callees` currently use a same-file static call graph. They are useful navigation signals, not full type-aware call hierarchy results.

For accuracy boundaries and current non-goals, see [Known limitations](docs/known-limitations.md).

## Development

```bash
cargo fmt
cargo test
```

## Release Builds

The `Release Build` workflow can be triggered manually or by pushing a `v*` tag. It builds Linux and macOS artifacts and uploads them as workflow artifacts. It does not create a GitHub Release yet.

## License

CodeInsight MCP Server is licensed under the Apache License 2.0.

The first MVP intentionally avoids external services such as Qdrant, pgvector, Neo4j, or Apache AGE. The default path must remain local, single-binary, and low configuration.
