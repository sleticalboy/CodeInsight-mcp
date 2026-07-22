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
  --repo-url "https://github.com/owner/repo" \
  --expected-first-read "entrypoint, router, or application setup area" \
  --install-method "Source" \
  --mcp-client "Codex" \
  --version "$(codeinsight version)" \
  --output-dir /tmp/codeinsight-external-beta-trial
```

For private repositories, add `--private-repo` and review the generated
redaction checklist before uploading artifacts.

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

Prioritize the first fix in this order:

1. `workflow_friction` that blocks the trial command or issue filing.
2. `route_miss` where the first selected file is wrong for a common task.
3. `overtrust_risk` in user-facing wording.
4. `route_near_miss` with a small, testable routing improvement.
