# Task Routing Matrix

Use the task routing matrix when you want to check whether one repository routes
different agent prompts to different first-read files.

This is a route-quality check, not a benchmark suite. It helps answer:

- Does a routing task start at routing code?
- Does an authentication task start at auth code?
- Does an authorization task start at permission or token boundary code?
- Does a settings task start at config code?
- Does a feature flag task start at rollout, toggle, or experiment code?
- Does a network task start at proxy, redirect, adapter, or transport code?
- Does a TLS task start at certificate verification or SSL transport code?
- Does a validation task start at schema, binding, parser, or serializer code?
- Does a startup task preserve the application entrypoint?
- Does a persistence task start at database, repository, or storage code?
- Does a debugging task start at error handling, retry, or timeout code?
- Does a coverage task start at test, spec, or regression code?
- Does an API handler task start at handler, controller, or endpoint code?
- Does a performance task start at cache, latency, or optimization code?
- Does an observability task start at logs, metrics, telemetry, or tracing code?
- Does a security task start at sanitization, secrets, or vulnerability code?
- Does a billing task start at payment, checkout, invoice, or subscription code?
- Does a frontend task start at UI, component, page, or layout code?
- Does a background task start at queue, worker, job, or scheduler code?
- Does a documentation task start at docs, guide, or usage example code?
- Does a request lifecycle task start at app dispatch, hooks, or response finalization code?
- Does a middleware task start at middleware registration or handler boundary code?

## Run

Run the default matrix:

```bash
scripts/task-routing-matrix.sh /path/to/repo
```

The default matrix covers routing, authentication, authorization/access-control,
settings, feature flag/rollout, network/proxy/redirect, TLS/certificate
verification, startup, persistence, validation/binding/serialization,
debug/retry/timeout, regression coverage, API handler, cache/performance,
observability/logging, security/sanitization, billing/payment, frontend
component, background job, documentation, request lifecycle, middleware, and
AI-agent first-read workflow prompts.

Run a custom matrix:

```bash
scripts/task-routing-matrix.sh /path/to/repo \
  --task "understand routing behavior" \
  --task "understand authentication behavior" \
  --task "understand authorization permissions" \
  --task "understand access control rules" \
  --task "understand application settings" \
  --task "understand feature flag rollout" \
  --task "understand proxy redirect transport" \
  --task "understand ssl certificate verification" \
  --task "understand json binding validation" \
  --task "understand startup flow" \
  --task "understand persistence behavior" \
  --task "debug retry timeout handling" \
  --task "find regression coverage" \
  --task "understand api handler behavior" \
  --task "understand cache performance latency" \
  --task "understand observability telemetry logs" \
  --task "understand security sanitization vulnerabilities" \
  --task "understand checkout subscription payment" \
  --task "understand frontend component rendering" \
  --task "understand background job queue" \
  --task "understand documentation usage" \
  --task "understand request lifecycle before after request handling" \
  --task "understand middleware behavior" \
  --output-dir /tmp/codeinsight-task-routing-matrix
```

When a large repository has a known subsystem, pass explicit seeds. They are
applied to each route in that matrix and preserved in the generated summary:

```bash
scripts/task-routing-matrix.sh /path/to/repo \
  --task "understand the known security sanitizer" \
  --file src/security.ts \
  --symbol sanitizeSecurityInput \
  --output-dir /tmp/codeinsight-task-routing-matrix-security
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
`TASK=FILE` or tab-separated `TASK<TAB>FILE` rows. TSV rows can also include
per-task explicit seeds as columns 3 and 4, plus optional seed evidence checks
as columns 5 and 6:

```text
TASK<TAB>EXPECTED_FIRST_FILE<TAB>SEED_FILE<TAB>SEED_SYMBOL<TAB>EXPECTED_SEED_STRATEGY<TAB>EXPECTED_FIRST_SEED_VALUE
```

```text
understand routing behavior	src/router.ts
understand authentication behavior	src/auth.ts
understand the known security sanitizer	src/security.ts	src/security.ts	sanitizeSecurityInput
understand authorization permissions	src/permissions.ts
understand access control rules	src/permissions.ts
understand feature flag rollout	src/feature_flags.ts
understand proxy redirect transport	src/network.ts
understand ssl certificate verification	src/tls_transport.ts
understand json binding validation	src/validation.ts
understand persistence behavior	src/database.ts
debug retry timeout handling	src/retry_transport.ts
find regression coverage	src/router.test.ts
understand api handler behavior	src/handler.ts
understand cache performance latency	src/cache.ts
understand observability telemetry logs	src/telemetry.ts
understand security sanitization vulnerabilities	src/security.ts
understand checkout subscription payment	src/billing.ts
understand frontend component rendering	src/component.tsx
understand background job queue	src/worker.ts
understand documentation usage	docs/usage.ts
understand request lifecycle before after request handling	src/application.ts
understand middleware behavior	src/middleware.ts
improve AI agent first-read routing quality evidence	src/agent_workflow.ts
inspect src/auth.ts before editing login behavior	src/auth.ts			auto_task_path	src/auth.ts
```

Then run:

```bash
scripts/task-routing-matrix.sh /path/to/repo \
  --expect-file ./route-expectations.tsv \
  --min-route-quality-score 80
```

JSON expectation files are also supported:

```json
[
  {
    "task": "understand routing behavior",
    "expected_first_file": "src/router.ts",
    "seed_file": "src/router.ts",
    "seed_symbol": "createRouter",
    "expected_seed_strategy": "explicit",
    "expected_first_seed_value": "createRouter"
  },
  {
    "task": "inspect src/auth.ts before editing login behavior",
    "expected_first_file": "src/auth.ts",
    "expected_seed_strategy": "auto_task_path",
    "expected_first_seed_value": "src/auth.ts"
  }
]
```

Expectation files automatically add their tasks to the matrix. Per-task seeds
are passed only to their own route. Global `--file` and `--symbol` seeds are
still applied to every task. `expected_seed_strategy` and
`expected_first_seed_value` are assertions only; they let CI prove automatic
task-path seed selection without passing an explicit seed.
Expectation failures return a non-zero exit code after writing the summary, so
the failed expected/actual pair is still available as an artifact.

Use `--min-route-quality-score` when the first file is correct but the route
still needs enough evidence to be trusted by an agent:

```bash
scripts/task-routing-matrix.sh /path/to/repo \
  --expect-file ./route-expectations.tsv \
  --min-route-quality-score 80
```

When a task falls below the threshold, the command returns a non-zero exit code
after writing `summary.json`. The summary includes:

- `quality_gate.min_route_quality_score`
- `quality_gate.status`
- `quality_gate.failure_count`
- `quality_gate.failures[].task`
- `quality_gate.failures[].first_file`
- `quality_gate.failures[].route_quality_score`
- `quality_gate.failures[].route_quality_decision_summary`

This gate complements expected first-file checks. `--expect-file` proves the
route starts in the intended owner file; `--min-route-quality-score` proves the
route carried enough local evidence, confidence, and verification guidance to be
safe for first-read automation.

Checked-in examples:

- [Django](task-routing-expectations/django.tsv)
- [Express](task-routing-expectations/express.tsv)
- [FastAPI](task-routing-expectations/fastapi.tsv)
- [Flask](task-routing-expectations/flask.tsv)
- [Gin](task-routing-expectations/gin.tsv)
- [Requests](task-routing-expectations/requests.tsv)
- [Streamlit](task-routing-expectations/streamlit.tsv)
- [Wouter](task-routing-expectations/wouter.tsv)

Run the pinned fast public matrices in one pass:

```bash
scripts/public-task-routing-matrix.sh
```

The default set uses pinned Express, FastAPI, Flask, Gin, Requests, Streamlit,
and Wouter commits so expectation files do not drift with upstream default
branches.

Django is available as a pinned heavyweight manual case. It is not part of the
default public matrix because indexing and routing the full repository is slower
than the fast snapshot cases:

```bash
scripts/public-task-routing-matrix.sh \
  --case django \
  --root django=/tmp/codeinsight-public-task-routing-matrix/repos/django-dca76b15c62a1118325b71678ce3235e2231198d \
  --output-dir /tmp/codeinsight-public-task-routing-matrix/django-manual
```

If you do not already have a local checkout, omit `--root` and the script will
fetch the pinned Django ref.

Use local checkouts when you want deterministic or offline reproduction:

```bash
scripts/public-task-routing-matrix.sh \
  --case express \
  --root express=/tmp/codeinsight-case-express \
  --output-dir /tmp/codeinsight-public-task-routing-matrix
```

The command writes:

- `task-routing-matrix.md`
- `summary.json`
- one `local-repo-evidence.md` per task
- one raw `agent-route.json` per task

`scripts/public-task-routing-matrix.sh` also prints a compact evidence summary
with case count, expectation pass count, selected lines, estimated tokens, max
impacted files, and the distinct first files selected for each public case.
The checked-in [public task routing matrix](public-task-routing-matrix.md)
captures the current pinned Express, FastAPI, Flask, Gin, Requests, Streamlit,
and Wouter result.
Refresh it with:

```bash
scripts/update-public-task-routing-matrix.sh
```

Before a PR, check whether the snapshot is current:

```bash
scripts/update-public-task-routing-matrix.sh --check
```

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
When `--min-route-quality-score` is used, it also includes:

- `quality_gate.min_route_quality_score`
- `quality_gate.status`
- `quality_gate.failure_count`
- `quality_gate.failures[]`

When `--expect` or `--expect-file` is used, it also includes:

- `expectations.status`
- `expectations.count`
- `expectations.checks[].task`
- `expectations.checks[].expected_first_file`
- `expectations.checks[].actual_first_file`
- `expectations.checks[].expected_seed_strategy`
- `expectations.checks[].actual_seed_strategy`
- `expectations.checks[].expected_first_seed_value`
- `expectations.checks[].actual_first_seed_value`
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
