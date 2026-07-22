# ip2region Adoption Comparison

This is a reproducible adoption case for CodeInsight as a local-first AI-agent
code context router. It uses ip2region as a public repository and
compares a blind first read of all indexed source lines with CodeInsight's
routed first-read context.

This is adoption evidence, not a controlled performance benchmark. The goal is
to show what an AI coding agent can read first before opening files broadly.

## Snapshot

- Repository: `https://github.com/lionsoul2014/ip2region.git`
- Commit: `1a29562c2ddab00e26609f401afa921ed89af263`
- Short commit: `1a29562`
- Task: `understand ip2region java search flow`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/update-adoption-case.sh ip2region`

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `19379` source lines |
| CodeInsight routed first-read | `641` source lines |
| Source lines avoided | `18738` |
| First-read reduction | `96.7%` |
| Read less | `30.2x` |
| Selected files | `8` |
| Selected ranges | `16` |
| Estimated tokens | `5924` |
| Impacted files | `39` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_task_match` |
| First seed source | `task_match` |
| First seed value | `binding/java/src/main/java/org/lionsoul/ip2region/service/Ip2Region.java` |
| Companion entrypoint | `binding/c/main.c` |
| First selected file | `binding/java/src/main/java/org/lionsoul/ip2region/service/Ip2Region.java` |
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
scripts/update-adoption-case.sh ip2region
```

Recreate this exact snapshot:

```bash
scripts/update-adoption-case.sh ip2region --commit 1a29562c2ddab00e26609f401afa921ed89af263
```

Generate a fresh comparison against the current ip2region default branch:

```bash
rm -rf /tmp/codeinsight-case-ip2region
git clone --depth 1 https://github.com/lionsoul2014/ip2region.git /tmp/codeinsight-case-ip2region
scripts/adoption-comparison.sh /tmp/codeinsight-case-ip2region \
  --task "understand ip2region java search flow" \
  --output-dir /tmp/codeinsight-adoption-case-ip2region
```

For exact snapshot comparison, check out the commit listed above before running
the script.

Artifacts written by the command:

- `/tmp/codeinsight-adoption-case-ip2region/evidence/adoption-comparison.md`
- `/tmp/codeinsight-adoption-case-ip2region/evidence/summary.json`
- `/tmp/codeinsight-adoption-case-ip2region/evidence/local-repo-evidence.json`
- `/tmp/codeinsight-adoption-case-ip2region/evidence/agent-route.json`

