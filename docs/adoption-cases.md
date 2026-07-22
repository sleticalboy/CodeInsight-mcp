# Adoption Cases

This page summarizes checked-in public repository adoption cases for
CodeInsight as a local-first AI-agent code context router. Each case compares a
blind first read of all indexed source lines with the first context pack selected
by `index_project -> project_overview -> context_pack -> impact_analysis`.

These cases are adoption evidence, not controlled performance benchmarks. They
show what an AI coding agent can read first before opening files broadly.

## Summary

| Case | Ecosystem | Task | Blind lines | Routed lines | Avoided lines | Reduction | Read less | Details |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Django | Python web framework | understand django URL routing behavior | `529,403` | `593` | `528,810` | `99.9%` | `892.8x` | [case](adoption-case-django.md) |
| Express | JavaScript web framework | understand express application routing behavior | `21,478` | `232` | `21,246` | `98.9%` | `92.6x` | [case](adoption-case-express.md) |
| Gin | Go web framework | understand gin engine routing behavior | `24,099` | `248` | `23,851` | `99.0%` | `97.2x` | [case](adoption-case-gin.md) |
| Memchr | Rust search library | understand memchr search implementation flow | `69,381` | `230` | `69,151` | `99.7%` | `301.7x` | [case](adoption-case-memchr.md) |
| Requests | Python HTTP library | understand requests session request flow | `12,032` | `651` | `11,381` | `94.6%` | `18.5x` | [case](adoption-case-requests.md) |
| ip2region | Multi-language IP lookup library | understand ip2region java search flow | `19,379` | `641` | `18,738` | `96.7%` | `30.2x` | [case](adoption-case-ip2region.md) |

Aggregate snapshot:

- Public repositories: `6`
- Blind first-read baseline: `675,772` source lines
- CodeInsight routed first-read: `2,595` source lines
- Source lines avoided before broad file reading: `673,177`
- Aggregate first-read reduction: `99.6%`
- Aggregate read-less ratio: `260.4x`
- Selected files: `39`
- Selected ranges: `78`
- Estimated tokens: `24,133`
- Impacted files reported before edits: `150`

## How To Read These Numbers

The baseline is the number of indexed source lines an agent could read if it
opened the repository broadly before forming a plan. The routed first-read
count is the source text selected by CodeInsight for the same task under the
token budget before broad file reading starts. The reduction and read-less
ratio describe first-read context routing, not runtime performance, parser
accuracy, or a claim that unselected code is irrelevant.

Use these cases as adoption evidence for agent workflow cost and focus. Final
code conclusions still need normal local verification with the IDE, LSP,
compiler, test runner, and language-specific tools.

## Route Evidence

| Case | Commit | Seed strategy | First selected file | First reading focus | Companion entrypoint | First suggested tool | Impact risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Django | `dca76b15c62a1118325b71678ce3235e2231198d` | `auto_task_match` | `django/urls/resolvers.py` | Start with seed file route registration, matching, or handler dispatch boundaries. | `scripts/archive_eol_stable_branches.py` | `file_outline` | `high` |
| Express | `ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4` | `auto_task_match` | `lib/express.js` | Start with seed file context and primary symbols. | `-` | `file_outline` | `high` |
| Gin | `34dac209ffb6ef85cc78c5d217bbb7ad001d68fd` | `auto_task_match` | `routergroup.go` | Start with seed file context and primary symbols. | `context.go` | `file_outline` | `high` |
| Memchr | `bce7df7140acff420478a358cde5587904000cb1` | `auto_task_match` | `benchmarks/engines/rust-memchr/main.rs` | Start with seed file context and primary symbols. | `benchmarks/engines/rust-jetscii/main.rs` | `file_outline` | `high` |
| Requests | `f361ead047be5cb873174218582f7d8b9fcd9f49` | `auto_task_match` | `src/requests/sessions.py` | Start with seed file context and primary symbols. | `src/requests/help.py` | `file_outline` | `high` |
| ip2region | `1a29562c2ddab00e26609f401afa921ed89af263` | `auto_task_match` | `binding/java/src/main/java/org/lionsoul/ip2region/service/Ip2Region.java` | Start with seed file context and primary symbols. | `binding/c/main.c` | `file_outline` | `high` |

## Refresh

Refresh checked-in snapshots:

```bash
scripts/update-adoption-case.sh django
scripts/update-adoption-case.sh express
scripts/update-adoption-case.sh gin
scripts/update-adoption-case.sh memchr
scripts/update-adoption-case.sh requests
scripts/update-adoption-case.sh ip2region
```

Generate the same shape for another repository:

```bash
scripts/adoption-comparison.sh /path/to/repo \
  --task "understand the app entrypoint" \
  --output-dir /tmp/codeinsight-adoption-comparison
```

