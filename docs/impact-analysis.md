# Impact Analysis

`impact_analysis` estimates a local change radius from indexed symbols, text references, static call edges, and resolved local dependencies. It is designed for AI-agent triage and review planning, not for language-server-grade type checking.

Run `index` first:

```bash
codeinsight index /path/to/repo
codeinsight impact-analysis /path/to/repo --symbol AuthService --file src/auth.py --depth 2 --format summary
```

The MCP tool accepts the same core arguments through `impact_analysis`: `root`, `symbols`, `files`, `limit`, `depth`, `format`, and `evidence_limit`.

## Output Contract

The report includes:

- `impacted_files`: ranked files with an aggregate `score` and up to 8 `reasons` per file.
- `paths`: explanatory multi-hop call/dependency paths, limited by `limit`.
- `risk_level`: one of `low`, `medium`, or `high`.
- `impact_counts`: counts from the full analysis before summary evidence truncation.
- `top_reasons`: up to 8 globally high-signal reasons, ranked by impacted file score.
- evidence arrays: `symbols`, `references`, `callers`, `callees`, `dependencies`, and `errors`.

`format=summary` keeps `impacted_files`, `paths`, `risk_level`, `impact_counts`, and `top_reasons` intact, while truncating evidence arrays to `evidence_limit`. `format=full` returns full evidence arrays up to `limit`.

## Score Weights

File scores are additive. A file can receive multiple reasons from different evidence sources.

| Evidence source | Score |
| --- | ---: |
| Seed file | 100 |
| Seed symbol definition | 90 |
| Symbol found inside a seed file | 80 |
| Text reference | 40 |
| Direct caller | 70 |
| Callee source file | 45 |
| Callee target file | 65 |
| Dependency source file | 55 |
| Dependency target file | 60 |

Multi-hop expansion decays with depth:

| Path source | Formula | Floor |
| --- | --- | ---: |
| Caller path | `70 - ((depth - 1) * 15)` | 20 |
| Dependency importer path | `60 - ((depth - 1) * 10)` | 20 |

Scores are stable enough for client ranking and compact summaries, but the exact values are still a v0.1 heuristic contract. Treat them as relative priority signals, not absolute risk probabilities.

## Risk Levels

`risk_level` is derived after `impacted_files` and `paths` are ranked and limited.

| Level | Trigger |
| --- | --- |
| `high` | At least 10 impacted files, or max file score at least 300, or max path depth at least 3 |
| `medium` | At least 4 impacted files, or max file score at least 160, or max path depth at least 2 |
| `low` | None of the medium or high triggers match |

Higher levels win. For example, a report with 3 files can still be `high` if one file accumulates a score of 300 or a path reaches depth 3.

## Current Limits

- Calls are derived from static syntax extraction and name matching.
- Text references can include non-semantic matches.
- Dependency paths depend on locally resolved imports only.
- `limit` can cap `impacted_files`, `paths`, and full evidence arrays, so very large repositories should use a generous limit for review planning.
