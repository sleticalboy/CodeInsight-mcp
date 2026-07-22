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
| Next.js app router | `vercel/next.js` | TypeScript web framework | `understand nextjs app router rendering flow` | App router rendering code | n/a | `workflow_friction` | Full-repo route generation was interrupted after repeated multi-minute runs; keep as a future large-repo filtering/performance probe. |

## Open Follow-Ups

- Collect at least three external user reports through the GitHub
  `Adoption feedback` issue form.
- Add one frontend or TypeScript routing adoption case that is small enough for
  the 10-minute Alpha trial path.
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

scripts/update-adoption-cases.sh --check
scripts/docs-smoke.sh
```
