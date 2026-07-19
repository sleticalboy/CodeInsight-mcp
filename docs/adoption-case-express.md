# Express Adoption Comparison

This is a reproducible adoption case for CodeInsight as a local-first AI-agent
code context router. It uses Express as a public repository and
compares a blind first read of all indexed source lines with CodeInsight's
routed first-read context.

This is adoption evidence, not a controlled performance benchmark. The goal is
to show what an AI coding agent can read first before opening files broadly.

## Snapshot

- Repository: `https://github.com/expressjs/express.git`
- Commit: `ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4`
- Short commit: `ae6dd37`
- Task: `understand express application routing behavior`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/update-adoption-case.sh express`

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `21478` source lines |
| CodeInsight routed first-read | `232` source lines |
| Source lines avoided | `21246` |
| First-read reduction | `98.9%` |
| Read less | `92.6x` |
| Selected files | `6` |
| Selected ranges | `7` |
| Estimated tokens | `1589` |
| Impacted files | `27` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_task_match` |
| First seed source | `task_match` |
| First seed value | `lib/express.js` |
| Companion entrypoint | `-` |
| First selected file | `lib/express.js` |
| First reading focus | Start with seed file context and primary symbols. |
| First suggested tool | `file_outline` |
| Impact risk | `high` |

First reading question:

```text
What entrypoints, exported symbols, or setup code define the main flow here?
```

## Reproduce

Refresh this checked-in snapshot:

```bash
scripts/update-adoption-case.sh express
```

Recreate this exact snapshot:

```bash
scripts/update-adoption-case.sh express --commit ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4
```

The legacy wrapper `scripts/update-adoption-case-express.sh` delegates to the same command.

Generate a fresh comparison against the current Express default branch:

```bash
rm -rf /tmp/codeinsight-case-express
git clone --depth 1 https://github.com/expressjs/express.git /tmp/codeinsight-case-express
scripts/adoption-comparison.sh /tmp/codeinsight-case-express \
  --task "understand express application routing behavior" \
  --output-dir /tmp/codeinsight-adoption-case-express
```

For exact snapshot comparison, check out the commit listed above before running
the script.

Artifacts written by the command:

- `/tmp/codeinsight-adoption-case-express/evidence/adoption-comparison.md`
- `/tmp/codeinsight-adoption-case-express/evidence/summary.json`
- `/tmp/codeinsight-adoption-case-express/evidence/local-repo-evidence.json`
- `/tmp/codeinsight-adoption-case-express/evidence/agent-route.json`

