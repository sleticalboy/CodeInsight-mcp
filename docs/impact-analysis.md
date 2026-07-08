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
- `suggested_checks`: command or review checks inferred from impacted languages, repository metadata, risk, paths, and errors.
- evidence arrays: `symbols`, `references`, `callers`, `callees`, `dependencies`, and `errors`.

`format=summary` keeps `impacted_files`, `paths`, `risk_level`, `impact_counts`, `top_reasons`, and `suggested_checks` intact, while truncating evidence arrays to `evidence_limit`. `format=full` returns full evidence arrays up to `limit`.

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

## Suggested Checks

`suggested_checks` is a compact list for review UIs and AI agents. Entries use:

- `kind`: `command` or `review`.
- `command`: present for runnable command candidates.
- `file`: present for file-specific review checks.
- `reason`: why the check was suggested.

Project configuration can override command checks. If `.codeinsight/config.toml` contains matching impact-analysis commands, those commands are emitted before review checks and built-in command inference is skipped. If no configured command matches, CodeInsight falls back to built-in command inference.

Create a starter config:

```bash
codeinsight init-config /path/to/repo
codeinsight init-config /path/to/repo --force
```

Without `--force`, `init-config` refuses to overwrite an existing config.

Example:

```toml
[impact_analysis]
test_commands = ["pnpm test"]

[[impact_analysis.suggested_checks]]
command = "pnpm exec vitest run src/core.test.ts"
reason = "Run the focused core test."
languages = ["typescript", "tsx"]
files = ["src/core"]
```

`test_commands` are global project commands. `suggested_checks` entries can filter by impacted `languages` and impacted file path prefixes in `files`. Empty filters match any impact report.

Built-in command checks are emitted only when impacted file languages match repository metadata:

| Impacted language | Repository signal | Suggested command |
| --- | --- | --- |
| Rust | `Cargo.toml` | `cargo test --locked` |
| JavaScript / TypeScript / TSX | `pnpm-lock.yaml` | `pnpm test` |
| JavaScript / TypeScript / TSX | `yarn.lock` | `yarn test` |
| JavaScript / TypeScript / TSX | `package-lock.json` or `package.json` | `npm test` |
| Python | `pyproject.toml`, `pytest.ini`, `setup.cfg`, `setup.py`, `tox.ini`, or `requirements.txt` | `pytest` |
| Go | `go.mod` | `go test ./...` |
| Java | `pom.xml` | `mvn test` |
| Java | `gradlew` | `./gradlew --no-daemon test` |
| Java | `build.gradle` or `build.gradle.kts` | `gradle test` |
| C# | root-level `.csproj` | `dotnet test` |
| Ruby | `Gemfile` | `bundle exec rspec` |
| PHP | `composer.json` | `composer test` |

Review checks are emitted for medium/high risk, multi-hop propagation paths, analysis errors, and the highest-ranked impacted file.

## Current Limits

- Calls are derived from static syntax extraction and name matching.
- Text references can include non-semantic matches.
- Dependency paths depend on locally resolved imports only.
- `limit` can cap `impacted_files`, `paths`, and full evidence arrays, so very large repositories should use a generous limit for review planning.
- Suggested commands are candidates, not guaranteed project scripts. Clients should let users edit or confirm them before execution.
- Configured commands take priority over built-in command inference, but review checks are still appended.
