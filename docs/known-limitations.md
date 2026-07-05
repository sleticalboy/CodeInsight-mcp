# Known Limitations

This document defines the accuracy boundaries for the current CodeInsight MCP Server MVP.

The project is intentionally local-first and lightweight. It uses Tree-sitter syntax parsing plus local SQLite indexes. It does not yet use language servers, compiler APIs, type checkers, embeddings, or external graph databases.

## Current Accuracy Model

### High Confidence

These capabilities are expected to be reliable for common source files in supported languages:

- Detecting supported source files by extension.
- Skipping noisy directories such as `.git`, `node_modules`, `target`, `dist`, and `.venv`.
- Extracting common symbol declarations:
  - Python classes and functions.
  - TypeScript/JavaScript classes, functions, interfaces, methods, and variables.
  - Go functions, methods, type declarations, consts, and vars.
  - Rust functions, structs, enums, traits, consts, statics, and simple impl methods.
- Caching indexed files by content hash.
- Skipping unchanged files during incremental indexing.
- Removing stale index records for deleted files.

### Medium Confidence

These capabilities are useful but approximate:

- Dependency extraction from imports and module declarations.
- `dependency_graph` output.
- Same-file `callers` and `callees` output.
- `context_pack` file/range selection.
- Reference classification as `definition`, `import`, `call`, or `text`.

The output is meant to guide an AI agent toward relevant files and line ranges, not to prove a complete semantic relationship.

### Low Confidence

These capabilities are intentionally basic in the MVP:

- Cross-file symbol resolution.
- Dynamic language call resolution.
- Method dispatch and inheritance resolution.
- Aliased imports.
- Re-export chains.
- Generated code awareness.
- Macro-expanded Rust code.
- Type-driven references.

## Tool-Specific Limitations

### `symbol_search`

`symbol_search` searches symbols that were extracted during indexing.

Limitations:

- It only returns declaration forms the extractor currently recognizes.
- It does not resolve overloaded symbols.
- It does not use type information.
- It may miss language-specific constructs not covered by the MVP extractor.

### `find_references`

`find_references` is currently a text-reference pass over indexed files.

Limitations:

- It is not equivalent to an IDE "find references" feature.
- It can return false positives from comments, tests, strings, and unrelated text.
- It can miss references through aliases, dynamic property access, reflection, macros, or generated code.
- `reference_kind` is inferred from line text and should be treated as approximate.

### `dependency_graph`

`dependency_graph` records module targets found in import-like syntax and resolves some local file targets.

Limitations:

- Targets are always stored as module strings; `resolved_file` is only populated when a local file can be resolved cheaply.
- Grouped imports may be compacted rather than expanded precisely.
- Package manager metadata is not analyzed yet.
- Monorepo workspace boundaries are not modeled yet.
- Go import paths are not resolved to files yet.

### `context_pack`

`context_pack` combines symbol search, reference search, and resolved local dependencies into a token-budgeted bundle.

Limitations:

- It is deterministic and local-only.
- It does not use semantic embeddings.
- It does not yet rank by call graph, type graph, test relevance, or edit history.
- Token estimation is approximate and based on character count.
- It may include noisy references when the seed symbol is common.

### `callers` and `callees`

`callers` and `callees` use a same-file static call graph extracted from call expressions.

Limitations:

- Calls are resolved by normalized callee name, not by type information.
- Cross-file calls are not resolved yet.
- Imported calls are not linked to their defining file yet.
- Dynamic dispatch, callbacks, reflection, macros, and higher-order functions are not modeled.
- Method calls with the same method name on different types may be conflated.

## Supported Languages

Current MVP support:

- Python
- JavaScript / TypeScript
- Go
- Rust

Rust support exists partly so the project can index itself during development.

Planned expansion:

- Java
- C / C++
- C#
- PHP or Ruby

## Non-Goals For v0.1.0

The following are explicitly out of scope for v0.1.0:

- Full LSP-compatible reference resolution.
- Compiler-grade type inference.
- Security taint analysis.
- Dead code analysis.
- Semantic vector search.
- Cross-repository knowledge graph.
- Web UI.
- Team/shared index service.

## Expected Usage Pattern

Use CodeInsight as an agent-facing navigation and context selection layer:

1. Index the repository.
2. Search symbols and inspect file outlines.
3. Use references and dependency graph as relevance signals.
4. Use `context_pack` to provide a compact starting context to an AI assistant.
5. Verify final code behavior with the project's real tests and build tools.

Do not treat current MVP output as a formal static-analysis proof.

## Improvement Roadmap

Near-term improvements:

- Store file-to-file resolved dependencies.
- Resolve imported calls where obvious.
- Expand `callers` and `callees` beyond same-file analysis.
- Improve import alias handling.
- Exclude or down-rank tests and comments in reference search.
- Add fixture repositories for each supported language.

Longer-term improvements:

- Optional language-server integration.
- Optional compiler metadata integration for strongly typed languages.
- Optional semantic search.
- PR impact analysis.
- Team index sharing.
