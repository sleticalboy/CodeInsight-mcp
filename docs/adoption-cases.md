# Adoption Cases

This page summarizes checked-in public repository adoption cases for
CodeInsight as a local-first AI-agent code context router. Each case compares a
blind first read of all indexed source lines with the first context pack selected
by `index_project -> project_overview -> context_pack -> impact_analysis`.

These cases are adoption evidence, not controlled performance benchmarks. They
show what an AI coding agent can read first before opening files broadly.

## Summary

| Case | Ecosystem | Task | Blind lines | Routed lines | Avoided lines | Reduction | Read less | Details |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Express | JavaScript web framework | understand express application routing behavior | `21,478` | `232` | `21,246` | `98.9%` | `92.6x` | [case](adoption-case-express.md) |
| Gin | Go web framework | understand gin engine routing behavior | `24,099` | `472` | `23,627` | `98.0%` | `51.1x` | [case](adoption-case-gin.md) |

Aggregate snapshot:

- Public repositories: `2`
- Blind first-read baseline: `45,577` source lines
- CodeInsight routed first-read: `704` source lines
- Source lines avoided before broad file reading: `44,873`
- Aggregate first-read reduction: `98.5%`
- Aggregate read-less ratio: `64.7x`
- Selected files: `12`
- Selected ranges: `28`
- Estimated tokens: `6,006`
- Impacted files reported before edits: `47`

## Route Evidence

| Case | Commit | Seed strategy | First selected file | Companion entrypoint | First suggested tool | Impact risk |
| --- | --- | --- | --- | --- | --- | --- |
| Express | `ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4` | `auto_task_match` | `lib/express.js` | `-` | `file_outline` | `high` |
| Gin | `34dac209ffb6ef85cc78c5d217bbb7ad001d68fd` | `auto_task_match` | `gin.go` | `context.go` | `file_outline` | `high` |

## Refresh

Refresh checked-in snapshots:

```bash
scripts/update-adoption-case.sh express
scripts/update-adoption-case.sh gin
```

Generate the same shape for another repository:

```bash
scripts/adoption-comparison.sh /path/to/repo \
  --task "understand the app entrypoint" \
  --output-dir /tmp/codeinsight-adoption-comparison
```

