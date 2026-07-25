# Task Coverage Evidence

This snapshot shows the task-critical file coverage gate added to
`scripts/adoption-comparison.sh`. It is adoption evidence for a real maintenance
task in this repository, not a parser-accuracy benchmark.

## Scenario

- Repository: `CodeInsight-mcp`
- Task: `add critical file gate to adoption comparison reports`
- Route: `index_project -> project_overview -> context_pack -> impact_analysis`
- Expected task-critical file: `scripts/adoption-comparison.sh`
- Command:

```bash
scripts/adoption-comparison.sh . \
  --task "add critical file gate to adoption comparison reports" \
  --file scripts/adoption-comparison.sh \
  --expected-file scripts/adoption-comparison.sh \
  --output-dir /tmp/codeinsight-adoption-comparison-critical-file-gate
```

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `82045` source lines |
| CodeInsight routed first-read | `541` source lines |
| Source lines avoided | `81504` |
| First-read reduction | `99.3%` |
| Read less | `151.7x` |
| Selected files | `3` |
| Selected ranges | `10` |
| Estimated tokens | `5812` |
| Impacted files | `49` |

## Task Coverage

| Field | Value |
| --- | --- |
| Coverage status | `pass` |
| Coverage | `1/1` |
| Expected selected files | `scripts/adoption-comparison.sh` |
| Routed selected files | `scripts/adoption-comparison.sh`, `scripts/adoption-evidence.sh`, `scripts/codebase-memory-backend-evidence.sh` |
| Missing expected files | `none` |

## Interpretation

The route did not only reduce first-read volume. It also selected the known
task-critical implementation file before editing. This is the evidence shape we
want for real user tasks: pass the files a maintainer already knows are critical
with `--expected-file`, then fail the adoption comparison if the routed first
read misses any of them.

This does not prove the selected context is complete. It proves the first-read
route included the declared task-critical files and remained bounded enough for
an AI agent to inspect before using follow-up tools.
