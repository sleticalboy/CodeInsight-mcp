# Task Routing Matrix

Use the task routing matrix when you want to check whether one repository routes
different agent prompts to different first-read files.

This is a route-quality check, not a benchmark suite. It helps answer:

- Does a routing task start at routing code?
- Does an authentication task start at auth code?
- Does a settings task start at config code?
- Does a startup task preserve the application entrypoint?

## Run

Run the default matrix:

```bash
scripts/task-routing-matrix.sh /path/to/repo
```

Run a custom matrix:

```bash
scripts/task-routing-matrix.sh /path/to/repo \
  --task "understand routing behavior" \
  --task "understand authentication behavior" \
  --task "understand application settings" \
  --task "understand startup flow" \
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

Expectation failures return a non-zero exit code after writing the summary, so
the failed expected/actual pair is still available as an artifact.

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
- first seed
- companion entrypoint
- selected source lines and reduction
- estimated tokens
- impact risk and impacted file count

The JSON summary is intended for CI artifacts and regression checks.
When `--expect` is used, it also includes:

- `expectations.status`
- `expectations.count`
- `expectations.checks[].task`
- `expectations.checks[].expected_first_file`
- `expectations.checks[].actual_first_file`
- `expectations.checks[].status`

## Example

Against the checked Gin adoption-case checkout, the task matrix routes distinct
questions to distinct first files:

| Task | First file | Seed strategy | Routed lines | Reduction | Tokens | Impact |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| understand gin engine routing behavior | `routergroup.go` | `auto_task_match` | `248/24099` | `99.0%` | `2122` | `high / 10` |
| understand middleware authentication behavior | `auth.go` | `auto_task_match` | `395/24099` | `98.4%` | `3871` | `high / 5` |
| understand startup flow | `gin.go` | `auto_entrypoint` | `305/24099` | `98.7%` | `2965` | `high / 20` |

This complements [Adoption cases](adoption-cases.md): adoption cases compare
blind first-read size with one routed first read, while this matrix checks
whether multiple prompts on the same repository pick the expected local start
points.
