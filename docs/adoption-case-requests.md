# Requests Adoption Comparison

This is a reproducible adoption case for CodeInsight as a local-first AI-agent
code context router. It uses Requests as a public repository and
compares a blind first read of all indexed source lines with CodeInsight's
routed first-read context.

This is adoption evidence, not a controlled performance benchmark. The goal is
to show what an AI coding agent can read first before opening files broadly.

## Snapshot

- Repository: `https://github.com/psf/requests.git`
- Commit: `f361ead047be5cb873174218582f7d8b9fcd9f49`
- Short commit: `f361ead`
- Task: `understand requests session request flow`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/update-adoption-case.sh requests`

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `12032` source lines |
| CodeInsight routed first-read | `651` source lines |
| Source lines avoided | `11381` |
| First-read reduction | `94.6%` |
| Read less | `18.5x` |
| Selected files | `12` |
| Selected ranges | `18` |
| Estimated tokens | `5937` |
| Impacted files | `4` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_task_match` |
| First seed source | `task_match` |
| First seed value | `src/requests/sessions.py` |
| Companion entrypoint | `src/requests/help.py` |
| First selected file | `src/requests/help.py` |
| First suggested tool | `file_outline` |
| Impact risk | `high` |

First reading question:

```text
What entrypoints, exported symbols, or setup code define the main flow here?
```

## Reproduce

Refresh this checked-in snapshot:

```bash
scripts/update-adoption-case.sh requests
```

Recreate this exact snapshot:

```bash
scripts/update-adoption-case.sh requests --commit f361ead047be5cb873174218582f7d8b9fcd9f49
```

Generate a fresh comparison against the current Requests default branch:

```bash
rm -rf /tmp/codeinsight-case-requests
git clone --depth 1 https://github.com/psf/requests.git /tmp/codeinsight-case-requests
scripts/adoption-comparison.sh /tmp/codeinsight-case-requests \
  --task "understand requests session request flow" \
  --output-dir /tmp/codeinsight-adoption-case-requests
```

For exact snapshot comparison, check out the commit listed above before running
the script.

Artifacts written by the command:

- `/tmp/codeinsight-adoption-case-requests/evidence/adoption-comparison.md`
- `/tmp/codeinsight-adoption-case-requests/evidence/summary.json`
- `/tmp/codeinsight-adoption-case-requests/evidence/local-repo-evidence.json`
- `/tmp/codeinsight-adoption-case-requests/evidence/agent-route.json`

