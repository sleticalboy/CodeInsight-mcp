# CodeInsight v0.1 Large Repository Benchmark

Generated at: 2026-07-15 02:02:25 UTC

This is a benchmark fixture report, not a controlled performance benchmark. It
verifies that CodeInsight can index real public repositories across the MVP
language set and produce stable project summaries and context packs without
crashing.

Environment:

- Command: `/Users/binlee/.cargo/target/release/codeinsight`
- Profile: `large`
- Work directory: temporary clone directory
- Index mode: forced clean index per repository
- Context pack mode: one stable file seed per repository, 6000 token budget
- Index budget mode: enabled

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Entrypoints | First entrypoint | Recommended tools | First recommended tool | Context files | Ranges | Context lines | Line reduction | Tokens | Applied budget | Omitted files | Continuation | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
| express | JavaScript | `ae6dd37` | 141 | 21478 | 2432 | 72 | 0 | 3101 | 10000 | pass | 6.2M | 12 | `examples/auth/index.js` | 4 | `context_pack` | 4 | 13 | 362 | 98.3% | 2360 | 6000 | 0 | complete | false | `lib/application.js` |
| flask | Python | `36e4a82` | 83 | 18337 | 1620 | 153 | 0 | 356 | 5000 | pass | 1.2M | 6 | `src/flask/cli.py` | 4 | `context_pack` | 12 | 15 | 573 | 96.9% | 5851 | 6000 | 2 | omitted_candidates_available | true | `src/flask/app.py` |
| gin | Go | `34dac20` | 99 | 24099 | 1857 | 31 | 0 | 418 | 5000 | pass | 1.8M | 3 | `gin.go` | 4 | `context_pack` | 4 | 12 | 305 | 98.7% | 2969 | 6000 | 0 | complete | false | `gin.go` |
| tokio | Rust | `dac81bf` | 790 | 177641 | 8472 | 75 | 0 | 2855 | 20000 | pass | 6.9M | 12 | `examples/chat.rs` | 4 | `context_pack` | 18 | 23 | 508 | 99.7% | 5054 | 6000 | 0 | complete | false | `tokio/src/lib.rs` |

## Details

## express

- URL: https://github.com/expressjs/express.git
- Commit: `ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4`
- Indexed files: 141
- Symbols: 2432
- Duration: 3101 ms
- Index budget: 10000 ms (pass)
- Entrypoint candidates: 12
- First entrypoint candidate: `examples/auth/index.js`
- Recommended next tools: 4
- Context seed file: `lib/application.js`
- Context task: understand express application routing behavior
- Context files: 4
- Context ranges: 13
- Context lines: 362 of 21478 (98.3% reduction)
- Context estimated tokens: 2360
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
- Context truncated: false

Entrypoint candidates:

| File | Symbol | Role | Confidence | Reason |
| --- | --- | --- | ---: | --- |
| `examples/auth/index.js` | `-` | example | 0.73 | conventional index file |
| `examples/content-negotiation/index.js` | `-` | example | 0.73 | conventional index file |
| `examples/cookie-sessions/index.js` | `-` | example | 0.73 | conventional index file |
| `examples/cookies/index.js` | `-` | example | 0.73 | conventional index file |
| `examples/downloads/index.js` | `-` | example | 0.73 | conventional index file |

Recommended next tools:

| Tool | Priority | Reason |
| --- | ---: | --- |
| `context_pack` | 10 | Build first-read context from indexed source files because no source entrypoint was detected. |
| `dependency_graph` | 30 | Inspect module and package relationships; the most frequent external target is Content-Type. |
| `callers` | 40 | Inspect static call graph edges because no source entrypoint was detected. |
| `config_status` | 80 | Check project-specific validation commands before planning changes. |

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `lib/application.js` | 10 | 1-7 | high |
| `lib/utils.js` | 1 | 1-40 | high |
| `lib/view.js` | 1 | 1-40 | high |
| `index.js` | 1 | 1-11 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| javascript | 141 | 21478 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 3 | 4 | pass |
| `selected_ranges` | >= 10 | 13 | pass |
| `reading_plan_steps` | >= 3 | 4 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `estimated_tokens` | <= 3000 and applied budget | 2360 / 6000 | pass |
| `line_reduction` | >= 95% | 98.3% | pass |

Symbol target guardrails:

| Target | Minimum symbols | Observed symbols | Status |
| --- | ---: | ---: | --- |
| `createError` | 1 | 14 | pass |
| `handleError` | 1 | 10 | pass |
| `User.index` | 1 | 1 | pass |
| `User.range` | 1 | 1 | pass |
| `users.list` | 1 | 1 | pass |
| `METHODS` | 1 | 1 | pass |
| `Buffer` | 1 | 13 | pass |
| `address` | 1 | 2 | pass |
| `port` | 1 | 1 | pass |

Call target guardrails:

| Target | Minimum calls | Observed calls | Status |
| --- | ---: | ---: | --- |
| `app.get` | 1 | 196 | pass |
| `app.<dynamic>` | 1 | 5 | pass |
| `app.route.get` | 1 | 2 | pass |
| `router.route.get` | 1 | 1 | pass |

Call edge guardrails:

| Caller | Callee | Minimum calls | Observed calls | Status |
| --- | --- | ---: | ---: | --- |
| `it.<callback>` | `app.route.get` | 1 | 2 | pass |
| `app.get.<callback>` | `res.send` | 1 | 52 | pass |

## flask

- URL: https://github.com/pallets/flask.git
- Commit: `36e4a824f340fdee7ed50937ba8e7f6bc7d17f81`
- Indexed files: 83
- Symbols: 1620
- Duration: 356 ms
- Index budget: 5000 ms (pass)
- Entrypoint candidates: 6
- First entrypoint candidate: `src/flask/cli.py`
- Recommended next tools: 4
- Context seed file: `src/flask/app.py`
- Context task: understand flask application dispatch behavior
- Context files: 12
- Context ranges: 15
- Context lines: 573 of 18337 (96.9% reduction)
- Context estimated tokens: 5851
- Context applied token budget: 6000
- Context omitted files: 2
- Context omitted ranges: 2
- Context truncation reason: token_budget_exhausted
- Context continuation status: omitted_candidates_available
- Context truncated: true

Entrypoint candidates:

| File | Symbol | Role | Confidence | Reason |
| --- | --- | --- | ---: | --- |
| `src/flask/cli.py` | `main` | source | 1.0 | entry symbol named main |
| `src/flask/app.py` | `run` | source | 0.8 | entry-like symbol named run |
| `tests/test_appctx.py` | `handler` | test | 0.71 | service entry-like symbol named handler |
| `tests/test_basic.py` | `handler` | test | 0.71 | service entry-like symbol named handler |
| `src/flask/sansio/app.py` | `-` | source | 0.68 | conventional app file |

Recommended next tools:

| Tool | Priority | Reason |
| --- | ---: | --- |
| `context_pack` | 10 | Build first-read context from the highest-confidence source entrypoint. |
| `dependency_graph` | 30 | Inspect dependency edges touching the source entrypoint src/flask/cli.py before deeper navigation. |
| `impact_analysis` | 40 | Estimate the entrypoint change radius using call and dependency signals. |
| `config_status` | 80 | Check project-specific validation commands before planning changes. |

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `src/flask/app.py` | 4 | 1-40 | high |
| `src/flask/globals.py` | 1 | 1-40 | high |
| `src/flask/__init__.py` | 1 | 1-39 | medium |
| `src/flask/cli.py` | 1 | 1-40 | medium |
| `src/flask/ctx.py` | 1 | 1-40 | medium |
| `src/flask/debughelpers.py` | 1 | 1-40 | medium |
| `src/flask/helpers.py` | 1 | 1-40 | medium |
| `src/flask/sansio/app.py` | 1 | 1-40 | medium |
| `src/flask/sessions.py` | 1 | 1-40 | medium |
| `src/flask/signals.py` | 1 | 1-17 | medium |
| `src/flask/templating.py` | 1 | 1-40 | medium |
| `src/flask/testing.py` | 1 | 1-40 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| python | 83 | 18337 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 8 | 12 | pass |
| `selected_ranges` | >= 10 | 15 | pass |
| `reading_plan_steps` | >= 6 | 8 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `estimated_tokens` | <= 6000 and applied budget | 5851 / 6000 | pass |
| `line_reduction` | >= 90% | 96.9% | pass |

## gin

- URL: https://github.com/gin-gonic/gin.git
- Commit: `34dac209ffb6ef85cc78c5d217bbb7ad001d68fd`
- Indexed files: 99
- Symbols: 1857
- Duration: 418 ms
- Index budget: 5000 ms (pass)
- Entrypoint candidates: 3
- First entrypoint candidate: `gin.go`
- Recommended next tools: 4
- Context seed file: `gin.go`
- Context task: understand gin engine routing behavior
- Context files: 4
- Context ranges: 12
- Context lines: 305 of 24099 (98.7% reduction)
- Context estimated tokens: 2969
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
- Context truncated: false

Entrypoint candidates:

| File | Symbol | Role | Confidence | Reason |
| --- | --- | --- | ---: | --- |
| `gin.go` | `Run` | source | 0.8 | entry-like symbol named Run |
| `ginS/gins.go` | `Run` | source | 0.8 | entry-like symbol named Run |
| `context.go` | `Handler` | source | 0.71 | service entry-like symbol named Handler |

Recommended next tools:

| Tool | Priority | Reason |
| --- | ---: | --- |
| `context_pack` | 10 | Build first-read context from the highest-confidence source entrypoint. |
| `dependency_graph` | 30 | Inspect dependency edges touching the source entrypoint gin.go before deeper navigation. |
| `impact_analysis` | 40 | Estimate the entrypoint change radius using call and dependency signals. |
| `config_status` | 80 | Check project-specific validation commands before planning changes. |

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `gin.go` | 9 | 1-7 | high |
| `internal/bytesconv/bytesconv.go` | 1 | 1-21 | high |
| `internal/fs/fs.go` | 1 | 1-21 | medium |
| `render/bson.go` | 1 | 1-34 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| go | 99 | 24099 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 3 | 4 | pass |
| `selected_ranges` | >= 10 | 12 | pass |
| `reading_plan_steps` | >= 3 | 4 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `estimated_tokens` | <= 3500 and applied budget | 2969 / 6000 | pass |
| `line_reduction` | >= 95% | 98.7% | pass |

## tokio

- URL: https://github.com/tokio-rs/tokio.git
- Commit: `dac81bf8c8de0a3e35f1626643674ba9faf9569c`
- Indexed files: 790
- Symbols: 8472
- Duration: 2855 ms
- Index budget: 20000 ms (pass)
- Entrypoint candidates: 12
- First entrypoint candidate: `examples/chat.rs`
- Recommended next tools: 4
- Context seed file: `tokio/src/lib.rs`
- Context task: understand tokio runtime public API
- Context files: 18
- Context ranges: 23
- Context lines: 508 of 177641 (99.7% reduction)
- Context estimated tokens: 5054
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
- Context truncated: false

Entrypoint candidates:

| File | Symbol | Role | Confidence | Reason |
| --- | --- | --- | ---: | --- |
| `examples/chat.rs` | `main` | example | 1.0 | entry symbol named main |
| `examples/connect-tcp.rs` | `main` | example | 1.0 | entry symbol named main |
| `examples/connect-udp.rs` | `main` | example | 1.0 | entry symbol named main |
| `examples/custom-executor-tokio-context.rs` | `main` | example | 1.0 | entry symbol named main |
| `examples/custom-executor.rs` | `main` | example | 1.0 | entry symbol named main |

Recommended next tools:

| Tool | Priority | Reason |
| --- | ---: | --- |
| `context_pack` | 10 | Build first-read context from indexed source files because no source entrypoint was detected. |
| `dependency_graph` | 30 | Inspect module and package relationships; the most frequent external target is std::pin::Pin. |
| `callers` | 40 | Inspect static call graph edges because no source entrypoint was detected. |
| `config_status` | 80 | Check project-specific validation commands before planning changes. |

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `tokio/src/lib.rs` | 3 | 1-1 | high |
| `tokio/src/sync/watch.rs` | 3 | 1-40 | high, medium |
| `tokio/src/sync/once_cell.rs` | 2 | 353-357 | high |
| `tokio/src/sync/barrier.rs` | 1 | 138-142 | high |
| `tokio/src/sync/mpsc/bounded.rs` | 1 | 1271-1275 | high |
| `tokio/src/sync/mutex.rs` | 1 | 654-658 | high |
| `tokio/src/blocking.rs` | 1 | 1-40 | medium |
| `tokio/src/future/mod.rs` | 1 | 1-28 | medium |
| `tokio/src/loom/mod.rs` | 1 | 1-14 | medium |
| `tokio/src/sync/Notify.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/batch_semaphore.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/mpsc/chan.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/mpsc/error.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/notify.rs` | 1 | 1-40 | medium |
| `tokio/src/task/coop/mod.rs` | 1 | 1-40 | medium |
| `tokio/src/util/mod.rs` | 1 | 1-40 | medium |
| `tokio/src/util/trace.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/tests/mod.rs` | 1 | 1-18 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| rust | 790 | 177641 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 12 | 18 | pass |
| `selected_ranges` | >= 18 | 23 | pass |
| `reading_plan_steps` | >= 6 | 8 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `estimated_tokens` | <= 5500 and applied budget | 5054 / 6000 | pass |
| `line_reduction` | >= 95% | 99.7% | pass |
