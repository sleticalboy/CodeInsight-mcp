# Benchmark Methodology

CodeInsight benchmark reports are reproducible evidence for the AI-agent
first-read workflow. They are fixture benchmarks, not controlled performance
claims.

## What The Benchmarks Prove

The checked-in reports verify that CodeInsight can:

- clone and index real public repositories without external indexing services
- produce `project_overview.recommended_next_tools`
- route broad first-read tasks to `context_pack`
- keep selected context inside a 6000-token budget
- include `reading_plan[].question`, `reading_plan[].reason`, and
  `reading_plan[].selection_reason`
- report continuation and truncation state when context is omitted
- keep guardrail failures visible in the generated report

The reports do not prove compiler-grade reference precision, absolute indexing
performance, or universal token savings across arbitrary tasks.

## Profiles

| Profile | Report | Repositories | Purpose |
| --- | --- | --- | --- |
| `smoke` | `docs/benchmark-v0.1.md` | p-limit, itsdangerous, Go example, memchr | Fast cross-language sanity check. |
| `large` | `docs/benchmark-large.md` | express, Flask, Gin, Tokio | Larger repository context-routing evidence. |
| `local` | `CODEINSIGHT_BENCH_OUTPUT` or `benchmark-local.md` | one local checkout | Shareable evidence for an arbitrary local repository. |

## Refresh Commands

Run all smoke repositories:

```bash
scripts/benchmark-smoke.sh
```

Run all large repositories:

```bash
CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke.sh
```

Run a subset without overwriting checked-in reports:

```bash
CODEINSIGHT_BENCH_REPOS=p-limit scripts/benchmark-smoke.sh
CODEINSIGHT_BENCH_PROFILE=large CODEINSIGHT_BENCH_REPOS=flask scripts/benchmark-smoke.sh
```

Run an arbitrary local repository without writing `.codeinsight` into the
source checkout:

```bash
CODEINSIGHT_BENCH_PROFILE=local \
  CODEINSIGHT_BENCH_LOCAL_ROOT=/path/to/repo \
  CODEINSIGHT_BENCH_LOCAL_CONTEXT_FILE=src/main.ts \
  CODEINSIGHT_BENCH_LOCAL_TASK="understand the app entrypoint" \
  CODEINSIGHT_BENCH_OUTPUT=/tmp/codeinsight-local-benchmark.md \
  scripts/benchmark-smoke.sh
```

Reuse existing clones during local iteration:

```bash
CODEINSIGHT_BENCH_REUSE_REPOS=1 scripts/benchmark-smoke.sh
```

Print the active profile configuration:

```bash
CODEINSIGHT_BENCH_PRINT_CONFIG=1 scripts/benchmark-smoke.sh
```

## Guardrails

Each repository profile defines minimum or maximum expectations for:

- `selected_files`
- `selected_ranges`
- `reading_plan_steps`
- `estimated_tokens`
- `line_reduction`

Every report also checks that:

- the first recommended tool is `context_pack`
- the first reading-plan question is present
- the first reading-plan reason is actionable
- the first selection reason is present
- index time stays under the configured budget

## Validation

After refreshing reports, run:

```bash
scripts/docs-benchmark-smoke.sh
scripts/benchmark-report-smoke.sh docs/benchmark-v0.1.md smoke
scripts/benchmark-report-smoke.sh docs/benchmark-large.md large
```

`scripts/docs-smoke.sh` includes these checks as part of the documentation
gate.
