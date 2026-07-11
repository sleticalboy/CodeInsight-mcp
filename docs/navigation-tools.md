# Navigation tools

This document covers the lightweight navigation tools exposed through both the
CLI and MCP server: `find_references`, `callers`, and `callees`.

These tools are designed for fast AI-agent triage over the local SQLite index.
They are navigation signals, not language-server-grade semantic proofs.

## `find_references`

CLI:

```bash
codeinsight find-references /path/to/repo AuthService --limit 100 --include-definitions
```

MCP tool: `find_references`

Arguments:

- `root`: absolute or working-directory-relative repository path.
- `symbol`: symbol or text token to search for.
- `limit`: maximum matches to return. Defaults to `100` in the CLI.
- `include_definitions`: include definition-like matches when true. Defaults to
  `false`.

Each result contains:

- `file`: repository-relative file path.
- `line` and `column`: 1-based source location.
- `context`: source line containing the match.
- `reference_kind`: approximate line-level classification, such as
  `definition`, `call`, `import`, `inheritance`, `assignment`, or `read`.
- `confidence`: ranking confidence from textual and file-path heuristics.

Current behavior:

- Matches are found by scanning indexed source files.
- Obvious comment-only and string-only matches are filtered before ranking.
- Test and fixture paths are downranked so production references are less
  likely to be hidden by low-value matches.
- `reference_kind` is inferred from local line text and should be treated as a
  hint.

## `callers` and `callees`

CLI:

```bash
codeinsight callers /path/to/repo helper --limit 50
codeinsight callees /path/to/repo AuthService.login --limit 50
```

MCP tools: `callers`, `callees`

Arguments:

- `root`: absolute or working-directory-relative repository path.
- `symbol`: caller or callee symbol name to query.
- `limit`: maximum call edges to return. Defaults to `50` in the CLI.

Each call edge contains:

- `file`: repository-relative source file containing the call.
- `caller`: extracted caller symbol.
- `callee`: extracted callee expression.
- `callee_file`: optional repository-relative target file hint.
- `language`: indexed source language.
- `line` and `column`: 1-based source location.
- `confidence`: confidence assigned by the static extractor and resolver.

Current behavior:

- Same-file calls are recorded by normalized callee name.
- Java method invocations and common call expressions are indexed.
- JavaScript and TypeScript imported targets can include `callee_file` when a
  local import, alias, namespace import, default import, re-export, CommonJS
  require, dynamic import alias, `baseUrl`/`paths` import, or local package
  export resolves to an indexed file with a matching symbol.
- Python member calls such as `audit.record()` can include `callee_file` when a
  local `from .` / `from ..` import resolves to an indexed file with a matching
  symbol.
- Rust scoped calls such as `audit::record()` can include `callee_file` when a
  local `use crate::`, `use self::`, or `use super::` import resolves to an
  indexed file with a matching symbol.
- Go package-qualified calls such as `auth.Login()` can include `callee_file`
  when a same-module import resolves to an indexed local package file with a
  matching symbol.
- Java class-qualified calls such as `AuthService.login()` and static imported
  calls such as `defaultName()` can include `callee_file` when the import
  resolves to an indexed local source file with a matching symbol.
- C# alias-qualified calls such as `Audit.Record()` and static imported calls
  such as `ClampName()` can include `callee_file` when the `using` directive
  resolves to an indexed local source file with a matching symbol.
- PHP scoped calls such as `AuditLog::record()` and imported function calls
  such as `audit_login()` can include `callee_file` when the `use` import
  resolves to an indexed local source file with a matching symbol.
- Ruby member calls such as `Audit.record()` can include `callee_file` when a
  local `require_relative` import resolves to an indexed local source file with
  a matching symbol.

## Boundaries

- The tools do not use type checking.
- Overloads, dynamic dispatch, reflection, generated code, macros, and runtime
  mutation can be missed.
- Broad symbol names can still produce noisy text references.
- Imported call target hints are best-effort and currently deepest for
  JavaScript and TypeScript.
- For accuracy boundaries across all MVP tools, see
  [Known limitations](known-limitations.md).
