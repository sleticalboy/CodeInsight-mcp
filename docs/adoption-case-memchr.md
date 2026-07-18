# Memchr Adoption Comparison

This is a reproducible adoption case for CodeInsight as a local-first AI-agent
code context router. It uses Memchr as a public repository and
compares a blind first read of all indexed source lines with CodeInsight's
routed first-read context.

This is adoption evidence, not a controlled performance benchmark. The goal is
to show what an AI coding agent can read first before opening files broadly.

## Snapshot

- Repository: `https://github.com/BurntSushi/memchr.git`
- Commit: `bce7df7140acff420478a358cde5587904000cb1`
- Short commit: `bce7df7`
- Task: `understand memchr search implementation flow`
- Token budget: `6000`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Generated with: `scripts/update-adoption-case.sh memchr`

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `69381` source lines |
| CodeInsight routed first-read | `230` source lines |
| Source lines avoided | `69151` |
| First-read reduction | `99.7%` |
| Read less | `301.7x` |
| Selected files | `7` |
| Selected ranges | `12` |
| Estimated tokens | `2561` |
| Impacted files | `9` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | `auto_task_match` |
| First seed source | `task_match` |
| First seed value | `benchmarks/engines/rust-memchr/main.rs` |
| Companion entrypoint | `benchmarks/engines/rust-jetscii/main.rs` |
| First selected file | `benchmarks/engines/rust-memchr/main.rs` |
| First suggested tool | `file_outline` |
| Impact risk | `high` |

First reading question:

```text
What entrypoints, exported symbols, or setup code define the main flow here?
```

## Reproduce

Refresh this checked-in snapshot:

```bash
scripts/update-adoption-case.sh memchr
```

Recreate this exact snapshot:

```bash
scripts/update-adoption-case.sh memchr --commit bce7df7140acff420478a358cde5587904000cb1
```

Generate a fresh comparison against the current Memchr default branch:

```bash
rm -rf /tmp/codeinsight-case-memchr
git clone --depth 1 https://github.com/BurntSushi/memchr.git /tmp/codeinsight-case-memchr
scripts/adoption-comparison.sh /tmp/codeinsight-case-memchr \
  --task "understand memchr search implementation flow" \
  --output-dir /tmp/codeinsight-adoption-case-memchr
```

For exact snapshot comparison, check out the commit listed above before running
the script.

Artifacts written by the command:

- `/tmp/codeinsight-adoption-case-memchr/evidence/adoption-comparison.md`
- `/tmp/codeinsight-adoption-case-memchr/evidence/summary.json`
- `/tmp/codeinsight-adoption-case-memchr/evidence/local-repo-evidence.json`
- `/tmp/codeinsight-adoption-case-memchr/evidence/agent-route.json`

