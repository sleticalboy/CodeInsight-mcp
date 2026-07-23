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
- Task: `understand the CodeInsight MCP server Rust entrypoint`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/adoption-report.sh`
- Refreshed on: `2026-07-23`
- Source summary: `/tmp/codeinsight-self-adoption-report/summary.json`
- Source manifest: `/tmp/codeinsight-self-adoption-report/manifest.json`

## Result

| Metric | Value |
| --- | ---: |
| Indexed files | `138` |
| Symbols | `1946` |
| Index errors | `0` |
| Entrypoints | `12` |
| Type-relation edges | `7` |
| Top type-relation target | `EmbeddingProvider` |
| Type-relation graph filter | `base_type` |
| Blind first-read baseline | `77878` source lines |
| CodeInsight routed first-read | `540` source lines |
| Source lines avoided | `77338` |
| First-read reduction | `99.3%` |
| Read less | `144.2x` |
| Selected files | `30` |
| Selected ranges | `32` |
| Estimated tokens | `4826` |
| Reading plan steps | `8` |
| Impacted files | `50` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_entrypoint` |
| First seed source | `overview_entrypoint` |
| First seed value | `src/main.rs` |
| Companion entrypoint | `-` |
| First selected file | `src/main.rs` |
| First next action | `inspect_seed_file` |
| First reading focus | Start with seed file context and primary symbols. |
| First suggested tool | `file_outline` |
| Impact risk | `high` |

First reading question:

```text
What entrypoints, exported symbols, or setup code define the main flow here?
```

## MCP First-Call Contract

| Contract | Value |
| --- | --- |
| Route quality | `high` (`100/100`, `7` evidence signals) |
| Route quality next action | `read_selected_context` |
| Reading order starts with selected context | `true` |
| Current reading step mirrors reading plan | `true` |
| First execution instruction carries read-less evidence | `true` |
| Current-step suggested tool matches the reading plan | `true` |
| Continuation is checked after selected context | `true` |
| Suggested tool executed through MCP `tools/call` | `true` |
| MCP impact status | `complete` |

The first MCP selected file and first reading-plan file were both
`src/main.rs`, and the executable suggested tool was
`file_outline` with an absolute `/Users/binlee/code/open-source/CodeInsight-mcp/src/main.rs` path.

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
- Selected context: `540/77878` source lines, `99.3%` reduction
- Source lines avoided: `77338`
- Read less: `144.2x`
- Seed strategy: `auto_entrypoint`
- Selected seeds: `1`
- First seed source: `overview_entrypoint`
- Companion entrypoint: `-`
- Type-relation edges: `7`
- Top type-relation target: `EmbeddingProvider`
- Type-relation graph filter: `base_type`
- First selected file: `src/main.rs`
- First reading focus: Start with seed file context and primary symbols.
- First reading question: What entrypoints, exported symbols, or setup code define the main flow here?
- MCP server: `codeinsight`
- MCP route quality: `high` (`100/100`, `7` evidence signals), next=`read_selected_context`
- MCP first-call contract: reading_order=`true`, current_reading_step=`true`, read_less_instruction=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`
- First-read gating: suggested_tool_after_selected_context=`true`, continuation_after_selected_context=`true`, impact_review_before_edits=`true`
- MCP suggested tool executed: `true`
- MCP impact status: `complete`
```

## Reproduce

Refresh this checked-in snapshot:

```bash
scripts/update-self-adoption-report.sh
```

Verify the checked-in snapshot is current:

```bash
scripts/update-self-adoption-report.sh --check
```

Run from a CodeInsight checkout:

```bash
rm -rf /tmp/codeinsight-self-adoption-report /tmp/codeinsight-self-adoption-report.tar.gz
scripts/adoption-report.sh . \
  --task "understand the CodeInsight MCP server Rust entrypoint" \
  --token-budget 6000 \
  --output-dir /tmp/codeinsight-self-adoption-report \
  --archive /tmp/codeinsight-self-adoption-report.tar.gz \
  --print-snippet
```

Expected summary lines:

```text
- Selected context: `540/77878` source lines, `99.3%` reduction
- Source lines avoided: `77338`
- Read less: `144.2x`
- MCP route quality: `high` (`100/100`, `7` evidence signals), next=`read_selected_context`
- MCP first-call contract: reading_order=`true`, current_reading_step=`true`, read_less_instruction=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`
- First-read gating: suggested_tool_after_selected_context=`true`, continuation_after_selected_context=`true`, impact_review_before_edits=`true`
```
