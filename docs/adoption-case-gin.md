# Gin Adoption Comparison

This is a reproducible adoption case for CodeInsight as a local-first AI-agent
code context router. It uses Gin as a public repository and
compares a blind first read of all indexed source lines with CodeInsight's
routed first-read context.

This is adoption evidence, not a controlled performance benchmark. The goal is
to show what an AI coding agent can read first before opening files broadly.

## Snapshot

- Repository: `https://github.com/gin-gonic/gin.git`
- Commit: `34dac209ffb6ef85cc78c5d217bbb7ad001d68fd`
- Short commit: `34dac20`
- Task: `understand gin engine routing behavior`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/update-adoption-case.sh gin`

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `24099` source lines |
| CodeInsight routed first-read | `248` source lines |
| Source lines avoided | `23851` |
| First-read reduction | `99.0%` |
| Read less | `97.2x` |
| Selected files | `4` |
| Selected ranges | `13` |
| Estimated tokens | `2122` |
| Impacted files | `10` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_task_match` |
| First seed source | `task_match` |
| First seed value | `routergroup.go` |
| Companion entrypoint | `context.go` |
| First selected file | `routergroup.go` |
| First suggested tool | `file_outline` |
| Impact risk | `high` |

First reading question:

```text
What entrypoints, exported symbols, or setup code define the main flow here?
```

## Reproduce

Refresh this checked-in snapshot:

```bash
scripts/update-adoption-case.sh gin
```

Recreate this exact snapshot:

```bash
scripts/update-adoption-case.sh gin --commit 34dac209ffb6ef85cc78c5d217bbb7ad001d68fd
```

Generate a fresh comparison against the current Gin default branch:

```bash
rm -rf /tmp/codeinsight-case-gin
git clone --depth 1 https://github.com/gin-gonic/gin.git /tmp/codeinsight-case-gin
scripts/adoption-comparison.sh /tmp/codeinsight-case-gin \
  --task "understand gin engine routing behavior" \
  --output-dir /tmp/codeinsight-adoption-case-gin
```

For exact snapshot comparison, check out the commit listed above before running
the script.

Artifacts written by the command:

- `/tmp/codeinsight-adoption-case-gin/evidence/adoption-comparison.md`
- `/tmp/codeinsight-adoption-case-gin/evidence/summary.json`
- `/tmp/codeinsight-adoption-case-gin/evidence/local-repo-evidence.json`
- `/tmp/codeinsight-adoption-case-gin/evidence/agent-route.json`
