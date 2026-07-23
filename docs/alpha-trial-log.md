# Alpha Trial Log

This log tracks Public Adoption Alpha feedback after the first public MVP gate.
It is intentionally small: one row per meaningful trial, with enough detail to
reproduce route quality and prioritize fixes.

## Current Trials

| Trial | Repository | Ecosystem | Task | Expected first read | Actual first selected file | Outcome | Action |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Django URL routing | `django/django` | Python web framework | `understand django URL routing behavior` | URL resolver and route matching code | `django/urls/resolvers.py` | `route_hit` | Checked in as [Django adoption case](adoption-case-django.md). |
| Express routing | `expressjs/express` | JavaScript web framework | `understand express application routing behavior` | Application/router wiring | `lib/express.js` | `route_hit` | Checked in as [Express adoption case](adoption-case-express.md). |
| Gin routing | `gin-gonic/gin` | Go web framework | `understand gin engine routing behavior` | Route group or engine routing code | `routergroup.go` | `route_hit` | Checked in as [Gin adoption case](adoption-case-gin.md). |
| ip2region Java search | `lionsoul2014/ip2region` | Multi-language IP lookup library | `understand ip2region java search flow` | Java search implementation | `binding/java/src/main/java/org/lionsoul/ip2region/service/Ip2Region.java` | `route_hit` | Checked in as [ip2region adoption case](adoption-case-ip2region.md). |
| Memchr search flow | `BurntSushi/memchr` | Rust search library | `understand memchr search implementation flow` | Search implementation entrypoint | `src/lib.rs` | `route_hit` | Checked in as [Memchr adoption case](adoption-case-memchr.md). |
| Requests session flow | `psf/requests` | Python HTTP library | `understand requests session request flow` | Session request implementation | `src/requests/sessions.py` | `route_hit` | Checked in as [Requests adoption case](adoption-case-requests.md). |
| Wouter route matching | `molefrog/wouter` | TypeScript frontend routing library | `understand wouter route matching flow` | Core Wouter route matching source | `packages/wouter/src/index.js` | `route_hit` | Fixed package-source routing priority and checked in as [Wouter adoption case](adoption-case-wouter.md). |
| Next.js app router | `vercel/next.js` | TypeScript web framework | `understand nextjs app router rendering flow` | App router rendering code | n/a | `workflow_friction` | Full-repo route generation was interrupted after repeated multi-minute runs; explicit `--file` / `--symbol` seeds are now available for targeted large-repo retry, while full-repo performance remains a future probe. |

## Maintainer-Run Cohort

The first Alpha Feedback Loop cohort uses maintainer-run trials to validate the
same issue-form path expected from external users. These reports are not a
substitute for external feedback, but they prove the intake, labels, evidence
bundle, and MCP first-call checks work on real repositories.

| Issue | Repository | Ecosystem | Task | First selected file | Evidence | Outcome |
| --- | --- | --- | --- | --- | --- | --- |
| [#1](https://github.com/sleticalboy/CodeInsight-mcp/issues/1) | `lionsoul2014/ip2region` | Multi-language IP lookup library | `understand ip2region java search flow` | `binding/java/src/main/java/org/lionsoul/ip2region/service/Ip2Region.java` | 641 of 19,379 lines, 96.7% reduction, 30.2x read-less | `route_hit` |
| [#2](https://github.com/sleticalboy/CodeInsight-mcp/issues/2) | `ravitemer/mcp-hub` | JavaScript MCP hub | `understand mcp hub server routing flow` | `src/utils/router.js` | 601 of 9,111 lines, 93.4% reduction, 15.2x read-less | `route_hit` |
| [#3](https://github.com/sleticalboy/CodeInsight-mcp/issues/3) | `sleticalboy/lazy-mcp-wrapper` | Go MCP wrapper | `understand lazy mcp wrapper daemon startup flow` | `cmd/lazy-mcp-wrapper/main.go` | 713 of 15,119 lines, 95.3% reduction, 21.2x read-less | `route_hit` |

## Fixes From Cohort

- `scripts/mcp-first-call-smoke.sh` no longer requires external repository
  `file_outline` results to contain a `main` symbol. The default built-in
  fixture still checks `main`, while external roots only need a valid non-empty
  outline. This fixed adoption evidence generation for mcp-hub and ip2region.
- External Beta and adoption evidence wrappers now pass repeatable explicit
  `--file` and `--symbol` seeds through local CLI and MCP first-call checks.
  This gives large repositories such as Next.js a targeted retry path when
  broad automatic routing is too slow for the 10-minute trial.
- Route-dispatch tasks now prefer task-named package source roots such as
  `packages/wouter/src` over demo app packages, turning the Wouter route
  matching trial from a near-miss on `packages/magazin/App.tsx` into a
  route-hit on `packages/wouter/src/index.js`.

## Open Follow-Ups

- Collect at least three non-maintainer external user reports through the
  GitHub `Adoption feedback` issue form or the
  [External Beta trial](external-beta-trial.md) wrapper.
- For every `route_miss` or `route_near_miss`, decide whether the fix belongs
  in task seeding, framework entrypoint hints, reading-plan wording, or
  limitations.

## Maintainer Commands

```bash
scripts/adoption-evidence.sh /path/to/repo \
  --task "<reported task>" \
  --output-dir /tmp/codeinsight-adoption-evidence \
  --print-snippet \
  --issue-template

scripts/external-beta-trial.sh /path/to/repo \
  --task "<reported task>" \
  --output-dir /tmp/codeinsight-external-beta-trial

scripts/update-adoption-cases.sh --check
scripts/docs-smoke.sh
```
