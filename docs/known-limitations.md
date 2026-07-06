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
- Same-file `callers` and `callees` output, plus obvious local JavaScript/TypeScript imported call target hints.
- `context_pack` file/range selection.
- Reference classification as `definition`, `import`, `call`, or `text`.

The output is meant to guide an AI agent toward relevant files and line ranges, not to prove a complete semantic relationship.

### Low Confidence

These capabilities are intentionally basic in the MVP:

- Dynamic language call resolution.
- Method dispatch and inheritance resolution.
- Cross-file symbol resolution outside explicit local import/export edges.
- Arbitrary-depth re-export chains.
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
- Local package self-reference `exports` resolution supports JSON-compatible `package.json` files with exact or single-wildcard string mappings.
- Dependency package `exports` resolution supports nearest `node_modules` packages with exact or single-wildcard mappings and common condition objects.
- TypeScript and JavaScript `baseUrl`/`paths` resolution supports JSON-compatible `tsconfig.json` and `jsconfig.json` files with exact or single-wildcard path mappings.
- Monorepo workspace boundaries are not modeled yet.
- Go import paths are not resolved to files yet.

### `context_pack`

`context_pack` combines symbol search, file seeds, reference search, and resolved local dependencies into a token-budgeted bundle.

Current deterministic ranking order:

- File seeds have the highest priority.
- File seed ranges include header/import context plus primary top-level symbols. Large symbol and merged ranges are capped to keep small budgets useful. If no primary symbols are found, `context_pack` falls back to the first 80 lines.
- Symbol definition ranges are next.
- Text references are ranked after definitions, with reference confidence as a small boost.
- Resolved local dependencies are included as supporting context after direct matches.
- Task keywords provide a lightweight boost when they match symbol names, file paths, reference context, or dependency targets.
- Ties are broken by total file score and then stable file path order.

Limitations:

- It is deterministic and local-only.
- It does not use semantic embeddings.
- It does not yet rank by call graph, type graph, test relevance, semantic similarity, or edit history.
- Task relevance is lexical only and uses simple ASCII keyword matching.
- Token estimation is approximate and based on character count.
- It may include noisy references when the seed symbol is common.

### `callers` and `callees`

`callers` and `callees` use a static call graph extracted from call expressions. Same-file calls are recorded by normalized callee name. JavaScript and TypeScript calls can also receive a `callee_file` hint when an obvious local import/export edge resolves to an indexed file with a matching symbol.

Currently supported JavaScript/TypeScript imported target hints:

- Named imports: `import { render } from "./ui"; render()`.
- Aliased named imports: `import { render as draw } from "./ui"; draw()`.
- CommonJS destructuring: `const { render: draw } = require("./ui"); draw()`.
- Direct CommonJS member calls: `require("./ui").render()`.
- Static-string computed CommonJS targets: `require("./" + "ui").render()`.
- Namespace imports and module aliases: `import * as ui from "./ui"; ui.render()` and `const ui = require("./ui"); ui.render()`.
- Static-string dynamic import aliases: `const ui = await import("./ui"); ui.render()`.
- Static-string dynamic import callback aliases: `import("./ui").then((ui) => ui.render())`.
- TypeScript and JavaScript `baseUrl`/`paths` imports when the target resolves to an indexed local file.
- Local package self-reference `exports` imports when the target resolves to an indexed local file.
- Default imports when the target has an indexed `export default` symbol.
- One-hop named/default re-exports, `export * from`, and `export * as`.
- Two-hop named/default re-export aliases and two-hop namespace re-export aliases.

Limitations:

- Calls are resolved by normalized callee name, not by type information.
- `callee_file` is a best-effort file hint, not a proof of the exact runtime function.
- Re-export following is intentionally bounded; arbitrary-depth barrel chains are not expanded.
- Dependency packages under `node_modules` are skipped during indexing by default, so package export resolution can populate `dependency_graph.resolved_file` without producing `callee_file` hints for those packages.
- Non-literal dynamic `import()`, external dynamic import handlers, variable-based `require(...)` targets, multi-wildcard aliases/exports, and bundler resolution are not modeled yet.
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

- Improve external dynamic import handlers and variable-based `require(...)` handling where obvious.
- Add broader TypeScript path alias coverage and package manager metadata handling.
- Use call graph hints in `context_pack` ranking.
- Exclude or down-rank tests and comments in reference search.
- Add fixture repositories for each supported language.

Longer-term improvements:

- Optional language-server integration.
- Optional compiler metadata integration for strongly typed languages.
- Optional semantic search.
- PR impact analysis.
- Team index sharing.
