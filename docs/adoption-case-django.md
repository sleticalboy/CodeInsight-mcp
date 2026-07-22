# Django Adoption Comparison

This is a reproducible adoption case for CodeInsight as a local-first AI-agent
code context router. It uses Django as a public repository and
compares a blind first read of all indexed source lines with CodeInsight's
routed first-read context.

This is adoption evidence, not a controlled performance benchmark. The goal is
to show what an AI coding agent can read first before opening files broadly.

## Snapshot

- Repository: `https://github.com/django/django.git`
- Commit: `dca76b15c62a1118325b71678ce3235e2231198d`
- Short commit: `dca76b1`
- Task: `understand django URL routing behavior`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/update-adoption-case.sh django`

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `529403` source lines |
| CodeInsight routed first-read | `593` source lines |
| Source lines avoided | `528810` |
| First-read reduction | `99.9%` |
| Read less | `892.8x` |
| Selected files | `2` |
| Selected ranges | `12` |
| Estimated tokens | `6000` |
| Impacted files | `50` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_task_match` |
| First seed source | `task_match` |
| First seed value | `django/urls/resolvers.py` |
| Companion entrypoint | `scripts/archive_eol_stable_branches.py` |
| First selected file | `django/urls/resolvers.py` |
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
scripts/update-adoption-case.sh django
```

Recreate this exact snapshot:

```bash
scripts/update-adoption-case.sh django --commit dca76b15c62a1118325b71678ce3235e2231198d
```

Generate a fresh comparison against the current Django default branch:

```bash
rm -rf /tmp/codeinsight-case-django
git clone --depth 1 https://github.com/django/django.git /tmp/codeinsight-case-django
scripts/adoption-comparison.sh /tmp/codeinsight-case-django \
  --task "understand django URL routing behavior" \
  --output-dir /tmp/codeinsight-adoption-case-django
```

For exact snapshot comparison, check out the commit listed above before running
the script.

Artifacts written by the command:

- `/tmp/codeinsight-adoption-case-django/evidence/adoption-comparison.md`
- `/tmp/codeinsight-adoption-case-django/evidence/summary.json`
- `/tmp/codeinsight-adoption-case-django/evidence/local-repo-evidence.json`
- `/tmp/codeinsight-adoption-case-django/evidence/agent-route.json`

