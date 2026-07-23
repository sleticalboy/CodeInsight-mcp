# Wouter Adoption Comparison

This is a reproducible adoption case for CodeInsight as a local-first AI-agent
code context router. It uses Wouter as a public repository and
compares a blind first read of all indexed source lines with CodeInsight's
routed first-read context.

This is adoption evidence, not a controlled performance benchmark. The goal is
to show what an AI coding agent can read first before opening files broadly.

## Snapshot

- Repository: `https://github.com/molefrog/wouter.git`
- Commit: `e74a8095601d028234e4bbd2dc9ef0849f5cea8f`
- Short commit: `e74a809`
- Task: `understand wouter route matching flow`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/update-adoption-case.sh wouter`

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `6766` source lines |
| CodeInsight routed first-read | `404` source lines |
| Source lines avoided | `6362` |
| First-read reduction | `94.0%` |
| Read less | `16.7x` |
| Selected files | `8` |
| Selected ranges | `14` |
| Estimated tokens | `3915` |
| Impacted files | `28` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_task_match` |
| First seed source | `task_match` |
| First seed value | `packages/wouter/src/index.js` |
| Companion entrypoint | `packages/magazin/index.tsx` |
| First selected file | `packages/wouter/src/index.js` |
| First reading focus | Start with seed file route registration, matching, or handler dispatch boundaries. |
| First suggested tool | `file_outline` |
| Impact risk | `high` |

First reading question:

```text
Where are routes registered, matched, and dispatched to handlers here?
```

## Reproduce

Refresh this checked-in snapshot:

```bash
scripts/update-adoption-case.sh wouter
```

Recreate this exact snapshot:

```bash
scripts/update-adoption-case.sh wouter --commit e74a8095601d028234e4bbd2dc9ef0849f5cea8f
```

Generate a fresh comparison against the current Wouter default branch:

```bash
rm -rf /tmp/codeinsight-case-wouter
git clone --depth 1 https://github.com/molefrog/wouter.git /tmp/codeinsight-case-wouter
scripts/adoption-comparison.sh /tmp/codeinsight-case-wouter \
  --task "understand wouter route matching flow" \
  --output-dir /tmp/codeinsight-adoption-case-wouter
```

For exact snapshot comparison, check out the commit listed above before running
the script.

Artifacts written by the command:

- `/tmp/codeinsight-adoption-case-wouter/evidence/adoption-comparison.md`
- `/tmp/codeinsight-adoption-case-wouter/evidence/summary.json`
- `/tmp/codeinsight-adoption-case-wouter/evidence/local-repo-evidence.json`
- `/tmp/codeinsight-adoption-case-wouter/evidence/agent-route.json`

