# CLI Usage

Use `codeinsight` after installing a release or running `cargo install --path .`.
During local development, replace `codeinsight` with `cargo run --`.

## Basic Loop

Print version information:

```bash
codeinsight version
codeinsight --version
```

Index a repository:

```bash
codeinsight index /path/to/repo --force
```

Print an overview:

```bash
codeinsight overview /path/to/repo
```

The overview is the first-stop repository briefing for agents. After `index`,
it returns a compact `summary`, language and directory distribution,
symbol-kind counts, dependency and call-graph summaries, entrypoint candidates
with heuristic reasons, roles, confidence scores, index metadata, and
recommended next tools.

## Symbol And File Navigation

Search symbols:

```bash
codeinsight symbols /path/to/repo AuthService
```

Print a file outline:

```bash
codeinsight outline /path/to/repo/src/auth.ts
```

Print local dependencies:

```bash
codeinsight dependency-graph /path/to/repo --limit 50
codeinsight dependency-graph /path/to/repo --file src/service.cpp --language cpp --limit 50
```

Find references:

```bash
codeinsight find-references /path/to/repo AuthService --include-definitions
```

Inspect the static call graph:

```bash
codeinsight callers /path/to/repo helper
codeinsight callees /path/to/repo AuthService.login
```

For reference and call-graph response fields, see
[Navigation tools](navigation-tools.md).

## Agent Context

Build an agent context pack from inferred entrypoints:

```bash
codeinsight context-pack /path/to/repo --task "understand app entrypoint" --token-budget 6000
```

Build context from explicit symbol or file seeds:

```bash
codeinsight context-pack /path/to/repo --task "understand auth flow" --symbol AuthService --token-budget 6000
codeinsight context-pack /path/to/repo --task "understand auth module" --file src/auth.ts --token-budget 6000
```

For the ranking and response contract, see
[First-read workflow](first-read-workflow.md).

## Impact And Project Config

Estimate local impact radius:

```bash
codeinsight impact-analysis /path/to/repo --symbol AuthService --file src/auth.py --depth 2 --format summary
```

Create and inspect project-specific validation command configuration:

```bash
codeinsight init-config /path/to/repo
codeinsight config-status /path/to/repo
```

For scoring, risk levels, and validation-command configuration, see
[Impact analysis](impact-analysis.md).

## Semantic Search

Build semantic chunks and query them with a configured provider:

```bash
CODEINSIGHT_EMBEDDING_PROVIDER=local-hash codeinsight semantic-index /path/to/repo
CODEINSIGHT_EMBEDDING_PROVIDER=local-hash codeinsight semantic-search /path/to/repo "auth flow"
```

Check provider and local semantic index status:

```bash
codeinsight embedding-status
codeinsight embedding-status /path/to/repo
```

For provider setup, batching, explain output, and external-provider
boundaries, see [Embedding providers](embedding-providers.md).

## MCP Server

Start the MCP stdio server:

```bash
codeinsight serve --transport stdio
```

For client configuration snippets and smoke testing, see
[MCP client configuration](mcp-client-config.md) and
[MCP client smoke test](mcp-client-smoke.md).
