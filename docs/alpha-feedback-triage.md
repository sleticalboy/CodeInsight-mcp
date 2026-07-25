# Alpha Feedback Triage

This document defines the feedback loop after Public Adoption Alpha. The goal
is to turn real first-read routing trials into reproducible fixes without
changing the product positioning.

CodeInsight remains a local-first MCP code context router for AI coding agents.
Triage should favor workflow proof, route quality, and clear limitations over
large platform features.

## Intake

Preferred intake path:

1. User runs `scripts/adoption-evidence.sh` with `--issue-template`.
2. User files the GitHub `Adoption feedback` issue form.
3. Maintainer copies the result into [Alpha trial log](alpha-trial-log.md).
4. Maintainer classifies the outcome and decides whether the fix is routing,
   workflow documentation, or limitation wording.

If a user cannot share code, accept a redacted issue with:

- repository type and language
- exact task text
- first selected file or redacted path shape
- expected first file or expected area
- route outcome category
- generated `summary.json` metrics with private paths removed

## Labels

Use these labels:

- `adoption-feedback`: every alpha trial report.
- `needs-triage`: external Beta reports that have not yet been classified by
  a maintainer.
- `route-hit`: first selected file was useful.
- `route-near-miss`: first selected file was close but not ideal.
- `route-miss`: first selected file was wrong for the task.
- `workflow-friction`: install, MCP config, prompt, or output shape blocked the
  trial.
- `overtrust-risk`: wording made best-effort navigation look like a proof.

## Priority

Fix in this order:

1. `workflow-friction` that prevents the 10-minute trial.
2. Low route quality that keeps first-read evidence below the cohort gate.
3. `route-miss` in common frameworks or high-signal public repositories.
4. `overtrust-risk` wording in README, docs, CLI, or MCP tool output.
5. `route-near-miss` where a small routing heuristic can improve first file
   selection.
6. `route-hit` reports that only need evidence aggregation.

## External Beta Intake

For non-maintainer feedback, prefer:

```bash
scripts/external-beta-trial.sh /path/to/repo \
  --task "<reported task>" \
  --output-dir /tmp/codeinsight-external-beta-trial
```

The generated `issue-body.md` is the copyable report. The generated
`redaction-checklist.md` is required for private repositories, and
`maintainer-triage.md` keeps the first classification step explicit.

External users may start with `needs_triage` when they cannot confidently
choose a route outcome. Maintainers should replace it with `route_hit`,
`route_near_miss`, `route_miss`, `workflow_friction`, or `overtrust_risk`
after reproducing or reviewing the attached evidence.

After aggregating at least three reports, run:

```bash
scripts/external-beta-fix-queue.sh \
  /tmp/codeinsight-external-beta-cohort.json \
  --output /tmp/codeinsight-external-beta-fix-queue.md \
  --check
```

Use the generated queue as the first maintainer work list before publishing the
cohort as success evidence.

## MCP First-Call Checks

The MCP first-call smoke is an adoption gate, not a fixture-specific assertion.
When it runs against the built-in temporary fixture, it may require the
`file_outline` result to include `main`. When it runs against a real repository
through `scripts/adoption-evidence.sh`, it must only require the suggested tool
to execute and return a valid outline for the selected file.

If a real repository fails only because the first selected file has no `main`
symbol, classify that as `workflow_friction` in the evidence pipeline, then fix
the smoke contract rather than changing the repository task or route result.

## Reproduction Checklist

For each non-trivial report, reproduce with:

```bash
scripts/adoption-evidence.sh /path/to/repo \
  --task "<reported task>" \
  --output-dir /tmp/codeinsight-adoption-evidence \
  --print-snippet \
  --issue-template
```

When the report is for a public repository and should become checked-in
evidence, prefer:

```bash
scripts/adoption-comparison.sh /path/to/repo \
  --task "<reported task>" \
  --output-dir /tmp/codeinsight-adoption-comparison
```

Record the commit, expected first read, actual first selected file, line
reduction, read-less ratio, and first suggested tool in the trial log.

## Large Repository Friction

If a public repository cannot finish the 10-minute trial path because clone,
indexing, or route generation is too slow, classify it as `workflow_friction`
instead of forcing it into adoption evidence.

Use this fallback order:

1. Reuse an existing clean checkout with `--root`.
2. Reuse `.codeinsight/` with `CODEINSIGHT_ADOPTION_COMPARE_FORCE_INDEX=0`.
3. Switch from a debug binary to a release binary.
4. Record the interrupted trial in the log with elapsed time and last known
   phase.
5. Choose a smaller public repository for checked-in evidence, then keep the
   large repository as a future filtering or performance probe.

## Done Criteria

A feedback item is done when one of these is true:

- a route/workflow/doc fix is merged with local CI passing
- the issue is converted into a checked-in adoption case
- the limitation is documented clearly enough to avoid over-trust
- the report is not reproducible and the missing evidence is listed

Do not close route misses only because the read-less ratio looks good. The
first selected file still needs to be useful for the reported task.
