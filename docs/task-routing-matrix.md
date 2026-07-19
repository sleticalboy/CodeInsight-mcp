# Task Routing Matrix

Use the task routing matrix when you want to check whether one repository routes
different agent prompts to different first-read files.

This is a route-quality check, not a benchmark suite. It helps answer:

- Does a routing task start at routing code?
- Does an authentication task start at auth code?
- Does an authorization task start at permission or token boundary code?
- Does a settings task start at config code?
- Does a startup task preserve the application entrypoint?
- Does a persistence task start at database, repository, or storage code?
- Does a debugging task start at error handling, retry, or timeout code?
- Does a coverage task start at test, spec, or regression code?
- Does an API handler task start at handler, controller, or endpoint code?
- Does a performance task start at cache, latency, or optimization code?
- Does a billing task start at payment, checkout, invoice, or subscription code?
- Does a frontend task start at UI, component, page, or layout code?
- Does a background task start at queue, worker, job, or scheduler code?
- Does a documentation task start at docs, guide, or usage example code?

## Run

Run the default matrix:

```bash
scripts/task-routing-matrix.sh /path/to/repo
```

The default matrix covers routing, authentication, authorization, settings, startup,
persistence, debug/retry/timeout, regression coverage, API handler,
cache/performance, billing/payment, frontend component, background job,
documentation, and middleware prompts.

Run a custom matrix:

```bash
scripts/task-routing-matrix.sh /path/to/repo \
  --task "understand routing behavior" \
  --task "understand authentication behavior" \
  --task "understand authorization permissions" \
  --task "understand application settings" \
  --task "understand startup flow" \
  --task "understand persistence behavior" \
  --task "debug retry timeout handling" \
  --task "find regression coverage" \
  --task "understand api handler behavior" \
  --task "understand cache performance latency" \
  --task "understand checkout subscription payment" \
  --task "understand frontend component rendering" \
  --task "understand background job queue" \
  --task "understand documentation usage" \
  --output-dir /tmp/codeinsight-task-routing-matrix
```

Use expectations when you already know the intended first-read file and want a
CI gate:

```bash
scripts/task-routing-matrix.sh /path/to/repo \
  --task "understand routing behavior" \
  --task "understand authentication behavior" \
  --expect "understand routing behavior=src/router.ts" \
  --expect "understand authentication behavior=src/auth.ts"
```

For longer matrices, put the expectations in a file. Line-based files can use
`TASK=FILE` or tab-separated `TASK<TAB>FILE` rows:

```text
understand routing behavior	src/router.ts
understand authentication behavior	src/auth.ts
understand authorization permissions	src/permissions.ts
understand persistence behavior	src/database.ts
debug retry timeout handling	src/errors.ts
find regression coverage	src/router.test.ts
understand api handler behavior	src/handler.ts
understand cache performance latency	src/cache.ts
understand checkout subscription payment	src/billing.ts
understand frontend component rendering	src/component.tsx
understand background job queue	src/worker.ts
understand documentation usage	docs/usage.ts
```

Then run:

```bash
scripts/task-routing-matrix.sh /path/to/repo \
  --expect-file ./route-expectations.tsv
```

JSON expectation files are also supported:

```json
[
  {
    "task": "understand routing behavior",
    "expected_first_file": "src/router.ts"
  }
]
```

Expectation files automatically add their tasks to the matrix.
Expectation failures return a non-zero exit code after writing the summary, so
the failed expected/actual pair is still available as an artifact.

Checked-in examples:

- [Express](task-routing-expectations/express.tsv)
- [Gin](task-routing-expectations/gin.tsv)
- [Requests](task-routing-expectations/requests.tsv)
- [Streamlit](task-routing-expectations/streamlit.tsv)

The command writes:

- `task-routing-matrix.md`
- `summary.json`
- one `local-repo-evidence.md` per task
- one raw `agent-route.json` per task

## Output Contract

Each task row reports:

- task prompt
- seed strategy
- first selected file
- first reading focus
- first reading question
- first seed
- companion entrypoint
- selected source lines and reduction
- estimated tokens
- impact risk and impacted file count

The JSON summary is intended for CI artifacts and regression checks.
When `--expect` or `--expect-file` is used, it also includes:

- `expectations.status`
- `expectations.count`
- `expectations.checks[].task`
- `expectations.checks[].expected_first_file`
- `expectations.checks[].actual_first_file`
- `expectations.checks[].status`

## Example

Against the checked Gin adoption-case checkout, the task matrix routes distinct
questions to distinct first files:

| Task | First file | Focus | Question | Seed strategy | Routed lines | Reduction | Tokens | Impact |
| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| understand gin engine routing behavior | `routergroup.go` | Start with seed file context and primary symbols. | What entrypoints, exported symbols, or setup code define the main flow here? | `auto_task_match` | `248/24099` | `99.0%` | `2122` | `high / 10` |
| understand middleware authentication behavior | `auth.go` | Start with seed file authentication and session boundaries. | Where are authentication decisions, credentials, or session boundaries handled here? | `auto_task_match` | `395/24099` | `98.4%` | `3871` | `high / 5` |
| understand startup flow | `gin.go` | Start with seed file startup and initialization flow. | What startup entrypoint or initialization sequence creates the requested flow? | `auto_entrypoint` | `305/24099` | `98.7%` | `2965` | `high / 20` |

Against a Streamlit checkout with Python backend and TypeScript frontend code,
the same matrix catches broad prompts that previously drifted to plausible but
less useful files:

| Task | First file | Question | Seed strategy | Routed lines | Reduction | Tokens | Impact |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| understand streamlit server startup flow | `lib/streamlit/web/bootstrap.py` | What startup entrypoint or initialization sequence creates the requested flow? | `auto_entrypoint` | `272/556097` | `100.0%` | `2810` | `high / 6` |
| understand configuration settings | `lib/streamlit/config.py` | Which configuration options, defaults, or environment inputs control the requested behavior? | `auto_task_match` | `610/556097` | `99.9%` | `6000` | `high / 4` |

This complements [Adoption cases](adoption-cases.md): adoption cases compare
blind first-read size with one routed first read, while this matrix checks
whether multiple prompts on the same repository pick the expected local start
points.
