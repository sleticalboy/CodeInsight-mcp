# CodeInsight Self Adoption Report

This is a reproducible adoption report snapshot for CodeInsight itself. It uses
the complete `scripts/adoption-report.sh` path, not only the shorter
blind-read comparison flow, so it verifies the uploadable `tar.gz` report
shape, issue template, aggregate summaries, raw JSON, and diagnostic logs.

This is adoption evidence, not a controlled performance benchmark. The goal is
to prove that the report bundle preserves the same first-read route and MCP
first-call contract that a client or issue triage flow needs.

## Snapshot

- Repository: `CodeInsight-mcp`
- Root: `/Users/binlee/code/open-source/CodeInsight-mcp`
- Task: `understand the main application entrypoint`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/adoption-report.sh`
- Refreshed on: `2026-07-18`
- Source summary: `/tmp/codeinsight-self-adoption-report/summary.json`
- Source manifest: `/tmp/codeinsight-self-adoption-report/manifest.json`

## Result

| Metric | Value |
| --- | ---: |
| Indexed files | `23` |
| Symbols | `934` |
| Index errors | `0` |
| Entrypoints | `7` |
| Blind first-read baseline | `28433` source lines |
| CodeInsight routed first-read | `439` source lines |
| First-read reduction | `98.5%` |
| Selected files | `10` |
| Selected ranges | `11` |
| Estimated tokens | `4386` |
| Reading plan steps | `8` |
| Impacted files | `11` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_entrypoint` |
| First seed source | `overview_entrypoint` |
| First seed value | `src/main.rs` |
| Companion entrypoint | `-` |
| First selected file | `src/main.rs` |
| First next action | `inspect_seed_file` |
| First suggested tool | `file_outline` |
| Impact risk | `high` |

First reading question:

```text
What entrypoints, exported symbols, or setup code define the main flow here?
```

## MCP First-Call Contract

| Contract | Value |
| --- | --- |
| Reading order starts with selected context | `true` |
| Current-step suggested tool matches the reading plan | `true` |
| Continuation is checked after selected context | `true` |
| Suggested tool executed through MCP `tools/call` | `true` |
| MCP impact status | `complete` |

The first MCP selected file and first reading-plan file were both
`src/main.rs`, and the executable suggested tool was `file_outline` with an
absolute `src/main.rs` path.

## Report Bundle

The generated archive was:

```text
/tmp/codeinsight-self-adoption-report.tar.gz
```

The archive manifest contained:

- `adoption-evidence.md`
- `summary.json`
- `issue-template.md`
- `local-repo-evidence.md`
- `local-repo-evidence.json`
- `agent-route.json`
- `mcp-first-call.json`
- `local-repo-evidence.out`
- `local-repo-evidence.err`
- `mcp-first-call.out`
- `mcp-first-call.err`
- `artifact-write.err`
- `manifest.json`

The generated manifest reported `status: pass` and listed the same 13 files
that are packaged in the archive.

## Generated Snippet

The `--print-snippet` output from the refreshed report was:

```text
# CodeInsight Adoption Evidence

- Status: `pass`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Selected context: `439/28433` source lines, `98.5%` reduction
- Seed strategy: `auto_entrypoint`
- Selected seeds: `1`
- First seed source: `overview_entrypoint`
- Companion entrypoint: `-`
- First selected file: `src/main.rs`
- First reading question: What entrypoints, exported symbols, or setup code define the main flow here?
- MCP server: `codeinsight`
- MCP first-call contract: reading_order=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`
- First-read gating: suggested_tool_after_selected_context=`true`, continuation_after_selected_context=`true`, impact_review_before_edits=`true`
- MCP suggested tool executed: `true`
- MCP impact status: `complete`
```

## Reproduce

Run from a CodeInsight checkout:

```bash
rm -rf /tmp/codeinsight-self-adoption-report /tmp/codeinsight-self-adoption-report.tar.gz
scripts/adoption-report.sh . \
  --task "understand the main application entrypoint" \
  --token-budget 6000 \
  --output-dir /tmp/codeinsight-self-adoption-report \
  --archive /tmp/codeinsight-self-adoption-report.tar.gz \
  --print-snippet
```

Expected summary lines:

```text
- Selected context: `439/28433` source lines, `98.5%` reduction
- MCP first-call contract: reading_order=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`
- First-read gating: suggested_tool_after_selected_context=`true`, continuation_after_selected_context=`true`, impact_review_before_edits=`true`
```
