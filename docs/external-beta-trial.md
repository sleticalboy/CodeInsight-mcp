# External Beta Trial

This is the first non-maintainer feedback path after Public Adoption Alpha.
Use it when asking an external user to try CodeInsight on one real repository
and file a reproducible routing report.

The goal is not to prove compiler-grade code understanding. The goal is to
learn whether the local-first agent route helps a real AI coding workflow read
the right files before broad scanning.

## Trial Command

From a CodeInsight checkout:

```bash
scripts/external-beta-trial.sh /path/to/repo \
  --task "understand the main application entrypoint" \
  --file "src/main.ts" \
  --repo-url "https://github.com/owner/repo" \
  --expected-first-read "entrypoint, router, or application setup area" \
  --install-method "Source" \
  --mcp-client "Codex" \
  --version "$(codeinsight version)" \
  --output-dir /tmp/codeinsight-external-beta-trial
```

For private repositories, add `--private-repo` and review the generated
redaction checklist before uploading artifacts.

For very large repositories, prefer an explicit seed when the expected area is
known:

```bash
scripts/external-beta-trial.sh /path/to/repo \
  --task "understand nextjs app router rendering flow" \
  --file "packages/next/src/server/app-render/app-render.tsx" \
  --symbol "renderToHTMLOrFlight" \
  --no-force-index \
  --output-dir /tmp/codeinsight-external-beta-trial
```

`--file` and `--symbol` are repeatable. They are passed through to the local
CLI route and MCP first-call check, so the trial can start from a known
subsystem instead of relying only on broad automatic seed selection.

If broad indexing is too noisy for the trial, add `.codeinsight/config.toml`
before running the script:

```toml
[index]
include = ["packages/api/**", "src/**"]
exclude = ["**/*.generated.ts", "fixtures/**"]
```

The generated local evidence includes the applied `index_scope_*` metrics,
including actual walk roots, so maintainers can tell whether the report measured
a full repository or a scoped subsystem.

## Generated Files

The command writes:

- `issue-body.md`: copyable GitHub issue body for an external Beta report.
- `beta-summary.json`: machine-readable wrapper around the adoption evidence.
- `redaction-checklist.md`: private repository upload checklist.
- `maintainer-triage.md`: maintainer-side classification note.
- `adoption-evidence.md`, `summary.json`, `agent-route.json`, and
  `mcp-first-call.json`: underlying first-read route evidence.

## Filing Feedback

Open the GitHub `Adoption feedback` issue form and paste `issue-body.md` into
the `CodeInsight result` field. Attach the generated folder or paste the
artifact paths listed in the issue body.

For private repositories, do not upload raw paths, raw snippets, or repository
URLs until `redaction-checklist.md` is complete. It is fine to file the issue
with `needs_triage` and a redacted first selected file if the route quality is
unclear.

## Outcome

External users may choose `needs_triage` when they are unsure how to classify
the result. Maintainers then reclassify the report as one of:

- `route_hit`
- `route_near_miss`
- `route_miss`
- `workflow_friction`
- `overtrust_risk`

## Maintainer Beta Goal

The next public signal should be at least three non-maintainer reports filed
through the GitHub issue form or copied from `issue-body.md`.

Aggregate those reports with:

```bash
scripts/external-beta-cohort-summary.sh \
  /tmp/codeinsight-external-beta-trial-1 \
  /tmp/codeinsight-external-beta-trial-2 \
  /tmp/codeinsight-external-beta-trial-3 \
  --output /tmp/codeinsight-external-beta-cohort.md \
  --json /tmp/codeinsight-external-beta-cohort.json \
  --min-route-quality-score 70 \
  --check
```

Each argument can be a trial output directory or a `beta-summary.json` file. In
`--check` mode the command fails until at least three reports are present and
none are still `needs_triage`. When `--min-route-quality-score` is set, check
mode also fails if any report route quality score falls below that threshold,
so low-confidence routes remain feedback instead of public success evidence.

Prioritize the first fix in this order:

1. `workflow_friction` that blocks the trial command or issue filing.
2. Low route quality that makes the first-read evidence too weak to publish.
3. `route_miss` where the first selected file is wrong for a common task.
4. `overtrust_risk` in user-facing wording.
5. `route_near_miss` with a small, testable routing improvement.

Generate a maintainer queue from the cohort JSON:

```bash
scripts/external-beta-fix-queue.sh \
  /tmp/codeinsight-external-beta-cohort.json \
  --output /tmp/codeinsight-external-beta-fix-queue.md \
  --json /tmp/codeinsight-external-beta-fix-queue.json \
  --check
```

The queue preserves the same priority order and points each item at its
`beta-summary.json`, issue body, and maintainer triage note.

Package the cohort summary and fix queue into one handoff folder with:

```bash
scripts/external-beta-cohort-report.sh \
  /tmp/codeinsight-external-beta-trial-1 \
  /tmp/codeinsight-external-beta-trial-2 \
  /tmp/codeinsight-external-beta-trial-3 \
  --output-dir /tmp/codeinsight-external-beta-handoff \
  --min-route-quality-score 70 \
  --check \
  --print-snippet
```

The handoff folder contains `external-beta-cohort.md`,
`external-beta-cohort-summary.json`, `external-beta-fix-queue.md`,
`external-beta-fix-queue.json`, `manifest.json`, and a short `README.md` with
the cohort status, route-quality gate, next action, and fix queue size. The
manifest records the handoff stage, inputs, options, output files, cohort
status, and fix queue status for automation. `--print-snippet` prints a compact
Markdown summary for a GitHub issue or discussion. See the
[External Beta handoff example](external-beta-handoff-example.md) for the
expected artifact shape.
