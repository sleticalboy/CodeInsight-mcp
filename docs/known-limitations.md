# Known Limitations

This document defines the accuracy boundaries for the current CodeInsight MCP Server MVP.

The project is intentionally local-first and lightweight. It uses Tree-sitter syntax parsing plus local SQLite indexes. It does not yet use language servers, compiler APIs, type checkers, enabled-by-default embeddings, or external graph databases.

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
  - Java classes, interfaces, enums, records, methods, constructors, fields, package declarations, and imports.
  - C/C++ functions, structs/classes, enums, typedefs, macro constants, local includes, and basic calls.
  - C# classes, interfaces, structs, enums, records, methods, constructors, properties, fields, using directives, and basic calls.
  - PHP classes, interfaces, traits, enums, functions, methods, properties, constants, namespace use declarations, and basic calls.
  - Ruby classes, modules, methods, singleton methods, constants, require directives, and basic calls.
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
- It filters obvious comment-only and string-only matches, but multiline language edge cases can still produce false positives.
- Test and fixture files are downranked with lower confidence, but they can still appear when they contain useful references or when few production references exist.
- It can still return false positives from unrelated text-like code, generated code, or broad symbol names.
- It can miss references through aliases, dynamic property access, reflection, macros, or generated code.
- `reference_kind` is inferred from line text and should be treated as approximate.

### `semantic_search`

`semantic_search` can query local semantic vectors for a configured embedding provider. `semantic_index` can build local source-text chunks and optional local-hash vectors.

Limitations:

- The embedding provider interface exists, but no provider is enabled by default.
- Local semantic chunks can be stored and used as deterministic lexical fallback hints in `context_pack`.
- `CODEINSIGHT_EMBEDDING_PROVIDER=local-hash` can build and query deterministic local vectors.
- `CODEINSIGHT_EMBEDDING_PROVIDER=ollama` can build and query vectors from a local Ollama `/api/embed` endpoint.
- `CODEINSIGHT_EMBEDDING_PROVIDER=openai` can build and query vectors through an OpenAI-compatible `/embeddings` endpoint; `embedding-status` reports API-key presence without exposing the key.
- External embedding requests are batched by `CODEINSIGHT_EMBEDDING_BATCH_SIZE`, defaulting to 64 chunks per request.
- Calls fail with a clear provider-configuration error until `CODEINSIGHT_EMBEDDING_PROVIDER=local-hash`, `CODEINSIGHT_EMBEDDING_PROVIDER=ollama`, or another supported backend is enabled, and fail with an empty-index error until `semantic-index` has generated vectors for that provider/model.
- Qdrant-backed retrieval is planned but not implemented yet. See [Embedding providers](embedding-providers.md).
- `context_pack` can use semantic vector matches when the configured provider/model has indexed vectors, then falls back to deterministic lexical, symbol, reference, dependency, call graph, and semantic chunk metadata signals.

### `dependency_graph`

`dependency_graph` records module targets found in import-like syntax and resolves some local file targets.

Limitations:

- Optional `files` filters match dependencies whose source file or resolved target file touches the requested file; optional `languages` filters match the indexed source language of the dependency edge.
- Output includes `summary`, `top_sources`, and `top_targets` computed across all matching dependency edges before `limit` and `offset` page the returned edge list. Use `page_size` and `has_more` to drive follow-up pages.
- Targets are always stored as module strings; `resolved_file` is only populated when a local file can be resolved cheaply.
- Grouped imports may be compacted rather than expanded precisely.
- Local package self-reference `exports` resolution supports JSON-compatible `package.json` files with exact, single-wildcard, or multi-wildcard string mappings, exact subpaths preferred over wildcard patterns, configurable common condition priority, array fallback targets with skipped `null` or external package entries, explicit `null` disabled subpaths, and matched conditional `null` or external package branches.
- Local `package.json#imports` resolution supports exact, single-wildcard, or multi-wildcard mappings to relative local files, exact entries preferred over wildcard patterns, configurable common condition priority, array fallback targets with skipped `null` or external package entries, and matched conditional `null` or external package branches that block later alias fallback.
- Same-repository workspace package `exports` resolution supports JSON-compatible array-form `package.json#workspaces`, Yarn-style `package.json#workspaces.packages`, and common `pnpm-workspace.yaml` package lists with exact-directory, single-segment wildcard, recursive `**`, and negated package patterns such as `!packages/legacy-*` or `!packages/legacy/**`.
- Relative `workspace:` dependency paths such as `workspace:../pkg` can resolve package aliases to local package `exports` targets.
- Workspace version protocols such as `workspace:*`, `workspace:^`, `workspace:~`, and `workspace:<semver>` resolve through same-repository workspace package discovery when the dependency name matches a workspace package.
- `catalog:` and `catalog:name` dependency versions are treated as external catalog references and are not resolved to same-repository workspace packages by name; `pnpm-workspace.yaml` `catalog` and `catalogs` metadata is parsed when available, and nearest `node_modules` package resolution may still resolve installed catalog dependencies.
- Dependency package `exports` resolution supports nearest `node_modules` packages with exact, single-wildcard, or multi-wildcard mappings, exact subpaths preferred over wildcard patterns, extensionless local targets resolved through known JavaScript/TypeScript extensions, configurable common condition priority, array fallback targets with skipped `null` or external package entries, explicit `null` disabled subpaths, matched conditional `null` or external package branches, and common `browser` string or object remaps, including remaps to alternate local files, skipped external package targets, skipped absolute or non-string remap values, disabled `false` entries, and object keys written as either `./path` or `path`.
- Package metadata fallback supports root package specifiers through local relative `module`, `main`, `types`, and `typings` targets, skipping external, absolute, or non-string field values, plus package subpaths resolved as package-relative files or index files when no explicit `exports` mapping disables the subpath.
- TypeScript and JavaScript `baseUrl`/`paths` resolution supports JSON-compatible `tsconfig.json` and `jsconfig.json` files with relative `extends` chains, exact, single-wildcard, or multi-wildcard path mappings, exact entries preferred over wildcard patterns, multiple fallback mappings, and directory index files.
- External `imports` targets are not modeled yet.
- Python absolute imports resolve obvious repository-root `.py` files and package directories through `__init__.py`, and relative `from .` / `from ..` imports resolve nearby package files with member-target fallback such as `.support.audit` to `support/audit.py` or `support.py`, plus package directories through `__init__.py`; absolute and relative `from ... import name as alias` forms, including parenthesized multi-line imports and submodule aliases, can provide direct imported-call hints; third-party imports are not resolved. Go import paths resolve same-module packages declared by the nearest `go.mod`, preferring ordinary implementation files over package `doc.go` and `_test.go` files; standard library, third-party, `replace`, and `vendor` paths are not resolved to files yet. C# `using` directives resolve obvious same-repository `.cs` files under common roots such as `src`, including alias/static using targets and namespace directories with representative files; namespace declarations stay unresolved in the dependency graph but can provide `callee_file` hints for same-namespace class-qualified calls; simple type bindings such as `UserService users` or `App.Services.UserService backupUsers` can provide instance-call hints when the type is fully qualified or found through a local `using` or namespace declaration; `System.*` and `Microsoft.*` imports are not resolved. Java imports resolve obvious same-repository class files under common source roots such as `src/main/java`, including static member imports that can fall back to the containing class file; wildcard imports stay unresolved in the dependency graph but can provide `callee_file` hints for explicit class-qualified calls, and package declarations can provide hints for same-package class-qualified calls. PHP namespace `use` imports resolve obvious same-repository files under common roots such as `src`, including `App\` PSR-4 style paths, grouped use declarations, and function imports when a matching file exists. Ruby `require_relative` imports resolve obvious neighboring or parent-relative `.rb` files, with or without an explicit `.rb` suffix; gem-style `require` imports are not resolved. Rust `mod` declarations prefer sibling `foo.rs` before `foo/mod.rs`, and `crate::`, `self::`, and `super::` use paths resolve obvious local module files such as `src/foo.rs`, nested module files, or `src/foo/mod.rs`; external crate imports are not resolved.
- C/C++ quoted local includes are resolved from the source file directory, explicit relative paths, or obvious repository-root subpaths; system includes such as `<stdio.h>` are recorded but not resolved.

### `context_pack`

`context_pack` combines symbol search, file seeds, reference search, static call graph hints, resolved local dependencies, optional semantic vector matches, and local semantic chunk fallback hints into a token-budgeted bundle.

Current ranking order:

- File seeds have the highest priority.
- File seed ranges include header/import context plus primary top-level symbols. Task-matching seed symbols get a small same-file ordering boost, large same-score merged ranges are capped, oversized seed ranges can be shortened to fit small budgets, and selected output ranges are trimmed to avoid duplicate lines before being returned in source order. If no primary symbols are found, `context_pack` falls back to the first 80 lines.
- Symbol definition ranges are next.
- Static call graph target files from seed symbols and seed file primary symbols are ranked after definitions. Bounded caller files are also included for seed symbols and small seed files.
- Text references are ranked after call graph targets, with reference confidence as a small boost.
- Semantic vector matches are ranked after references when a configured provider/model has indexed vectors. Local semantic chunks remain available as deterministic fallback matches when their text matches task or seed symbol terms.
- Resolved local dependencies are included as supporting context after direct matches.
- Task keywords provide a lightweight boost when they match symbol names, file paths, reference context, or dependency targets.
- Inferred ranges from test and fixture files receive a low-value penalty across symbol, reference, dependency, and call-graph candidates by default. They are promoted instead when the task asks for tests, specs, coverage, regression, or when an explicit seed file is test-like.
- `reading_plan` is derived from the final selected files after token-budget selection. It is an ordered client hint, not a separate ranking pass. Its `next_action` values and `suggested_tool` calls are heuristic routing hints, not proof that the corresponding graph or dependency view is complete.
- `budget`, `omitted_candidates`, and `continuation_summary` explain how the selected context was budgeted and how a client can continue. They are continuation hints, not proof that every relevant file or range has been discovered.
- Ties are broken by total file score and then stable file path order.
- File-level `source` and `reason` report the dominant selected source among `seed_file`, `symbol_definition`, `reference`, `call_graph`, `semantic`, and `dependency`. File-level `score` is the highest selected range score, and range-level `source` and `score` report each selected range's source and score.
- `semantic_status` reports semantic candidate counts, selected semantic range counts, provider/model status, and a client-facing recommendation.

Limitations:

- Without a configured embedding provider, it remains deterministic and local-only.
- It uses vector embeddings only when the selected provider/model already has local indexed vectors; otherwise it keeps the deterministic fallback path.
- It does not yet rank by type graph, test relevance, semantic similarity, or edit history.
- Task relevance is lexical only and uses simple ASCII keyword matching.
- Token estimation is approximate and based on character count.
- It may include noisy references when the seed symbol is common.

### `callers` and `callees`

`callers` and `callees` use a static call graph extracted from call expressions and Java method invocations. Same-file calls are recorded by normalized callee name. JavaScript, TypeScript, Python, Rust, Go, Java, C#, PHP, Ruby, C, and C++ calls can also receive a `callee_file` hint when an obvious local import/export/include edge resolves to an indexed file with a matching symbol.

Currently supported JavaScript/TypeScript imported target hints:

- Named imports: `import { render } from "./ui"; render()`.
- Aliased named imports: `import { render as draw } from "./ui"; draw()`.
- CommonJS destructuring: `const { render: draw } = require("./ui"); draw()`.
- Direct CommonJS member calls: `require("./ui").render()`.
- Static-string computed CommonJS targets: `require("./" + "ui").render()`.
- Namespace imports and module aliases: `import * as ui from "./ui"; ui.render()` and `const ui = require("./ui"); ui.render()`.
- Static-string dynamic import aliases: `const ui = await import("./ui"); ui.render()`.
- Static-string dynamic import callback aliases: `import("./ui").then((ui) => ui.render())`.
- TypeScript and JavaScript `baseUrl`/`paths` imports, including aliases inherited through relative config `extends`, when the target resolves to an indexed local file.
- Local `package.json#imports` aliases when the target resolves to an indexed local file.
- Local package self-reference `exports` imports when the target resolves to an indexed local file.
- Same-repository `package.json`, `pnpm-workspace.yaml`, relative `workspace:` path, and workspace version protocol dependency package `exports` imports when the target resolves to an indexed local file; `catalog:` dependencies intentionally skip local workspace resolution.
- Default imports when the target has an indexed `export default` symbol.
- One-hop named/default re-exports, `export * from`, and `export * as`.
- Two-hop named/default re-export aliases and two-hop namespace re-export aliases.
- Python relative `from .` / `from ..` imports can provide `callee_file` hints for member calls such as `audit.record()` when the resolved local file contains the called member symbol.
- Python absolute `from ... import name as alias` imports can provide `callee_file` hints for direct alias calls such as `shared_ping()` when the resolved local package file contains the imported symbol, and for submodule alias calls such as `shared_tools.pong()` when the imported submodule resolves to a local file.
- Rust local `use crate::`, `use self::`, and `use super::` imports can provide `callee_file` hints for scoped calls such as `audit::record()` and direct imported calls when the resolved local file contains the called symbol.
- Go same-module imports can provide `callee_file` hints for package-qualified calls such as `auth.Login()` and explicit import aliases when the resolved local package representative file contains the called symbol.
- Java same-repository class, static, and wildcard imports can provide `callee_file` hints for class-qualified calls such as `AuthService.login()`, wildcard-imported class calls such as `Report.log()`, same-package class calls such as `LocalFormatter.decorate()`, and static imported calls such as `defaultName()` when the resolved local file contains the called symbol.
- C# same-repository alias and static `using` directives can provide `callee_file` hints for qualified calls such as `Audit.Record()` and static imported calls such as `ClampName()` when the resolved local file contains the called symbol; namespace declarations can provide hints for same-namespace class-qualified calls such as `LocalFormatter.Normalize()`; simple and fully-qualified type bindings can provide hints for instance calls such as `users.Find()` and `backupUsers.Find()`.
- PHP same-repository class, grouped class, function, and grouped function `use` imports can provide `callee_file` hints for scoped calls such as `AuditLog::record()` and imported function calls such as `audit_login()` when the resolved local file contains the called symbol.
- Ruby `require_relative` imports, including parent-relative paths such as `../support/audit.rb`, can provide `callee_file` hints for member calls such as `Audit.record()` when the resolved local file contains the called member symbol.
- C/C++ quoted local includes can provide `callee_file` hints for calls such as `shared_value()` or `declared_value()` when the resolved header contains an indexed inline function definition or simple function prototype declaration.

Limitations:

- Calls are resolved by normalized callee name, not by type information.
- `callee_file` is a best-effort file hint, not a proof of the exact runtime function.
- Re-export following is intentionally bounded; arbitrary-depth barrel chains are not expanded.
- Dependency packages under `node_modules` are skipped during indexing by default, so package export resolution can populate `dependency_graph.resolved_file` without producing `callee_file` hints for those packages.
- Non-literal dynamic `import()`, external dynamic import handlers, variable-based `require(...)` targets, package-based config `extends`, pnpm/yarn-specific workspace protocols, and broader bundler-specific resolution are not modeled yet.
- Dynamic dispatch, callbacks, reflection, macros, and higher-order functions are not modeled.
- Method calls with the same method name on different types may be conflated.

## Supported Languages

Current MVP support:

- Python
- JavaScript / TypeScript
- Go
- Rust
- Java
- C / C++
- C#
- PHP
- Ruby

Rust support exists partly so the project can index itself during development.

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
- Add advanced workspace glob and JavaScript package metadata edge-case handling.
- Use broader graph hints in `context_pack` ranking.
- Exclude or down-rank tests and comments in reference search.
- Add fixture repositories for each supported language.

Longer-term improvements:

- Optional language-server integration.
- Optional compiler metadata integration for strongly typed languages.
- Optional semantic search.
- PR impact analysis.
- Team index sharing.
